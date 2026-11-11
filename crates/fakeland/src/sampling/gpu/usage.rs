//! The DRM client usage statistics in `/proc/<pid>/fdinfo/<fd>` of an open DRM file. The kernel
//! format is `key:<tab>value` lines, with the keys:
//!
//! - `drm-driver` names the driver; `drm-pdev` the device; `drm-client-id` the client, which
//!   several file descriptors and processes can share.
//! - `drm-resident-<region>`, `drm-total-<region>` and the older `drm-memory-<region>` give memory
//!   per region with an optional `KiB`, `MiB` or `GiB` unit.
//! - `drm-engine-<engine>` gives the busy time of an engine in `ns`. `drm-engine-capacity-<engine>`
//!   gives the engine count of a class and carries no time.
//! - `drm-cycles-<engine>` and `drm-total-cycles-<engine>` give busy and elapsed GPU cycles.
//!
//! The memory of a client is the sum over regions of the resident memory, else of the total
//! memory, else of the older memory keys.

use std::collections::BTreeMap;

/// The usage statistics of one DRM client.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DrmClient {
    /// The driver, for example `amdgpu`, `i915` or `xe`.
    pub driver: String,
    /// The device, for example `0000:03:00.0`.
    pub pdev: Option<String>,
    /// The client id.
    pub client_id: Option<u64>,
    /// The resident memory over every region in bytes.
    pub resident: Option<u64>,
    /// The total memory over every region in bytes.
    pub total: Option<u64>,
    /// The memory of the older `drm-memory-<region>` keys in bytes.
    pub legacy: Option<u64>,
    /// The busy time of each engine in nanoseconds.
    pub engines: BTreeMap<String, u64>,
    /// The busy cycles of each engine.
    pub cycles: BTreeMap<String, u64>,
    /// The elapsed cycles of each engine.
    pub total_cycles: BTreeMap<String, u64>,
}

impl DrmClient {
    /// Returns the memory of the client in bytes.
    pub fn memory(&self) -> Option<u64> {
        self.resident.or(self.total).or(self.legacy)
    }

    /// Returns the busy time over every engine in nanoseconds.
    pub fn busy_ns(&self) -> Option<u64> {
        (!self.engines.is_empty()).then(|| self.engines.values().sum())
    }
}

/// Parses an fdinfo text. Returns `None` for a file without a `drm-driver` key.
pub fn parse_fdinfo(content: &str) -> Option<DrmClient> {
    let mut c = DrmClient::default();
    let mut driver = None;
    for line in content.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        let Some(key) = key.strip_prefix("drm-") else {
            continue;
        };
        match key {
            "driver" => driver = Some(value.to_string()),
            "pdev" => c.pdev = Some(value.to_string()),
            "client-id" => c.client_id = value.parse().ok(),
            _ => field(&mut c, key, value),
        }
    }
    c.driver = driver?;
    Some(c)
}

fn field(c: &mut DrmClient, key: &str, value: &str) {
    if key.starts_with("engine-capacity-") {
        return;
    }
    if let Some(engine) = key.strip_prefix("total-cycles-") {
        if let Ok(v) = value.parse() {
            c.total_cycles.insert(engine.to_string(), v);
        }
    } else if let Some(engine) = key.strip_prefix("cycles-") {
        if let Ok(v) = value.parse() {
            c.cycles.insert(engine.to_string(), v);
        }
    } else if let Some(engine) = key.strip_prefix("engine-") {
        if let Some(ns) = value.strip_suffix("ns").and_then(|v| v.trim().parse().ok()) {
            c.engines.insert(engine.to_string(), ns);
        }
    } else if key.starts_with("resident-") {
        add(&mut c.resident, parse_size(value));
    } else if key.starts_with("total-") {
        add(&mut c.total, parse_size(value));
    } else if key.starts_with("memory-") {
        add(&mut c.legacy, parse_size(value));
    }
}

fn add(sum: &mut Option<u64>, v: Option<u64>) {
    if let Some(v) = v {
        *sum = Some(sum.unwrap_or(0) + v);
    }
}

/// Parses a memory size: a number with an optional `KiB`, `MiB` or `GiB` unit.
pub fn parse_size(value: &str) -> Option<u64> {
    let mut parts = value.split_whitespace();
    let n: u64 = parts.next()?.parse().ok()?;
    let scale = match parts.next() {
        None => 1,
        Some("KiB") => 1 << 10,
        Some("MiB") => 1 << 20,
        Some("GiB") => 1 << 30,
        Some(_) => return None,
    };
    Some(n * scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMDGPU: &str = "pos:\t0\nflags:\t02100002\nmnt_id:\t26\nino:\t1154\n\
        drm-driver:\tamdgpu\ndrm-client-id:\t14\ndrm-pdev:\t0000:03:00.0\npasid:\t32790\n\
        drm-total-cpu:\t0\ndrm-resident-cpu:\t0\ndrm-total-gtt:\t2392 KiB\n\
        drm-resident-gtt:\t2392 KiB\ndrm-total-vram:\t34944 KiB\ndrm-shared-vram:\t0\n\
        drm-resident-vram:\t34944 KiB\ndrm-purgeable-vram:\t0\ndrm-memory-vram:\t34944 KiB\n\
        drm-memory-gtt: \t2392 KiB\ndrm-memory-cpu: \t0 KiB\namd-evicted-vram:\t0 KiB\n\
        drm-engine-gfx:\t1192583 ns\ndrm-engine-compute:\t1000 ns\ndrm-engine-dec:\t0 ns\n";

    const AMDGPU_OLD: &str = "pos:\t0\nflags:\t02100002\ndrm-driver:\tamdgpu\n\
        drm-pdev:\t0000:03:00.0\ndrm-client-id:\t7\ndrm-memory-vram:\t1 MiB\n\
        drm-memory-gtt:\t512 KiB\ndrm-memory-cpu:\t0 KiB\ndrm-engine-gfx:\t500 ns\n";

    const I915: &str = "pos:\t0\nflags:\t0100002\ndrm-driver:\ti915\ndrm-client-id:\t18\n\
        drm-pdev:\t0000:00:02.0\ndrm-total-system0:\t6144 KiB\ndrm-shared-system0:\t0\n\
        drm-active-system0:\t0\ndrm-resident-system0:\t6144 KiB\n\
        drm-purgeable-system0:\t0\ndrm-total-stolen-system0:\t0\n\
        drm-resident-stolen-system0:\t0\ndrm-engine-render:\t25662044495 ns\n\
        drm-engine-copy:\t0 ns\ndrm-engine-video:\t0 ns\ndrm-engine-capacity-video:\t2\n\
        drm-engine-video-enhance:\t0 ns\n";

    const XE: &str = "drm-driver:\txe\ndrm-client-id:\t10\ndrm-pdev:\t0000:00:02.0\n\
        drm-total-system:\t2056 KiB\ndrm-resident-system:\t2056 KiB\ndrm-total-gtt:\t1 MiB\n\
        drm-resident-gtt:\t1 MiB\ndrm-cycles-rcs:\t28257900\n\
        drm-total-cycles-rcs:\t7655183225\ndrm-cycles-bcs:\t0\n\
        drm-total-cycles-bcs:\t7655183225\ndrm-engine-capacity-vcs:\t2\n";

    #[test]
    fn parses_amdgpu() {
        let c = parse_fdinfo(AMDGPU).unwrap();
        assert_eq!(c.driver, "amdgpu");
        assert_eq!(c.pdev.as_deref(), Some("0000:03:00.0"));
        assert_eq!(c.client_id, Some(14));
        assert_eq!(c.resident, Some((2392 + 34944) << 10));
        assert_eq!(c.memory(), Some((2392 + 34944) << 10));
        assert_eq!(c.busy_ns(), Some(1_193_583));
        assert!(c.cycles.is_empty());
    }

    #[test]
    fn parses_older_amdgpu_memory_keys() {
        let c = parse_fdinfo(AMDGPU_OLD).unwrap();
        assert_eq!((c.resident, c.total), (None, None));
        assert_eq!(c.memory(), Some((1 << 20) + (512 << 10)));
        assert_eq!(c.busy_ns(), Some(500));
    }

    #[test]
    fn parses_i915_and_skips_engine_capacity() {
        let c = parse_fdinfo(I915).unwrap();
        assert_eq!(c.driver, "i915");
        assert_eq!(c.memory(), Some(6144 << 10));
        assert_eq!(c.engines.len(), 4);
        assert_eq!(c.busy_ns(), Some(25_662_044_495));
    }

    #[test]
    fn parses_xe_cycles_apart_from_total_memory() {
        let c = parse_fdinfo(XE).unwrap();
        assert_eq!(c.driver, "xe");
        assert_eq!(c.total, Some((2056 << 10) + (1 << 20)));
        assert_eq!(c.memory(), Some((2056 << 10) + (1 << 20)));
        assert_eq!(c.busy_ns(), None);
        assert_eq!(c.cycles["rcs"], 28_257_900);
        assert_eq!(c.total_cycles["bcs"], 7_655_183_225);
    }

    #[test]
    fn rejects_files_without_a_driver() {
        assert_eq!(parse_fdinfo("pos:\t0\nflags:\t02\n"), None);
        assert_eq!(parse_size("3 KiB"), Some(3072));
        assert_eq!(parse_size("7"), Some(7));
        assert_eq!(parse_size("2 GiB"), Some(2 << 30));
        assert_eq!(parse_size("5 pages"), None);
    }
}
