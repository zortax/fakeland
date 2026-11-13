//! Device-wide energy counters. Each domain is a cumulative counter in microjoules:
//!
//! - `cpu-package-<n>`: the top-level package zones of powercap, `/sys/class/powercap/intel-rapl:<n>`
//!   with a `package-<n>` name. AMD CPUs publish the same zones. Many kernels let only root read
//!   `energy_uj`. The counter wraps at `max_energy_range_uj`, and the domain adds the wrapped range.
//! - `gpu-hwmon-<n>`: `energy1_input` of a hwmon directory of the render device, as amdgpu
//!   publishes on some GPUs.
//! - `gpu-nvml-<index>`: the NVML energy of the device with the `nvml` feature.
//!
//! A domain whose counter is unreadable at open is left out.

use std::fs;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::gpu::GpuContext;

/// The powercap zones.
const POWERCAP: &str = "/sys/class/powercap";

/// One energy counter at one sample.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnergyReading {
    /// The domain, for example `cpu-package-0` or `gpu-nvml-0`.
    pub domain: String,
    /// The energy since the counter start in microjoules.
    pub microjoules_total: u64,
}

/// A counter that wraps at a range, extended to a total that never wraps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unwrap {
    last: Option<u64>,
    total: u64,
}

impl Unwrap {
    /// Adds a raw value and returns the total. A raw value below the last one adds the range.
    pub fn update(&mut self, raw: u64, range: u64) -> u64 {
        self.total = match self.last {
            None => raw,
            Some(last) if raw >= last => self.total + (raw - last),
            Some(last) => self.total + range.saturating_sub(last) + raw,
        };
        self.last = Some(raw);
        self.total
    }
}

#[derive(Debug)]
enum Source {
    Powercap {
        path: PathBuf,
        range: u64,
        unwrap: Unwrap,
    },
    Hwmon(PathBuf),
    #[cfg(feature = "nvml")]
    Nvml(super::gpu::NvmlDevice),
}

impl Source {
    /// Returns true when a read stalls other GPU work.
    fn stalls(&self) -> bool {
        match self {
            Self::Powercap { .. } | Self::Hwmon(_) => false,
            #[cfg(feature = "nvml")]
            Self::Nvml(_) => true,
        }
    }

    fn read(&mut self) -> Option<u64> {
        match self {
            Self::Powercap {
                path,
                range,
                unwrap,
            } => Some(unwrap.update(read_number(path)?, *range)),
            Self::Hwmon(path) => read_number(path),
            #[cfg(feature = "nvml")]
            Self::Nvml(dev) => dev.energy_mj().map(|mj| mj * 1000),
        }
    }
}

/// The energy domains of the host.
#[derive(Debug, Default)]
pub struct EnergyMeter {
    domains: Vec<(String, Source)>,
}

impl EnergyMeter {
    /// Finds every readable domain.
    pub fn open(ctx: &GpuContext) -> Self {
        let mut domains = powercap_domains(Path::new(POWERCAP));
        if let Some(dev) = &ctx.device {
            domains.extend(hwmon_domains(&dev.dir));
        }
        #[cfg(feature = "nvml")]
        if let Some(dev) = ctx.nvml.clone() {
            domains.push((format!("gpu-nvml-{}", dev.index()), Source::Nvml(dev)));
        }
        domains.retain_mut(|(name, s)| {
            let ok = s.read().is_some();
            if !ok {
                tracing::debug!("energy domain {name} is unreadable");
            }
            ok
        });
        Self { domains }
    }

    /// Returns the domain names.
    pub fn names(&self) -> Vec<&str> {
        self.domains.iter().map(|d| d.0.as_str()).collect()
    }

    /// Reads every domain. A failed read leaves its domain out of this sample. The NVML counter
    /// stalls other GPU work of the host for several milliseconds, so it is read only when `slow`
    /// is true.
    pub fn read(&mut self, slow: bool) -> Vec<EnergyReading> {
        self.domains
            .iter_mut()
            .filter(|(_, s)| slow || !s.stalls())
            .filter_map(|(domain, s)| {
                Some(EnergyReading {
                    domain: domain.clone(),
                    microjoules_total: s.read()?,
                })
            })
            .collect()
    }
}

/// Returns the top-level package zones under a powercap directory.
fn powercap_domains(root: &Path) -> Vec<(String, Source)> {
    let Ok(dir) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Source)> = dir
        .flatten()
        .filter_map(|e| {
            let file = e.file_name();
            let zone = file.to_str()?.strip_prefix("intel-rapl:")?;
            if zone.contains(':') {
                return None;
            }
            let path = e.path();
            let name = fs::read_to_string(path.join("name")).ok()?;
            let name = name.trim();
            name.starts_with("package").then(|| {
                let source = Source::Powercap {
                    path: path.join("energy_uj"),
                    range: read_number(&path.join("max_energy_range_uj")).unwrap_or(0),
                    unwrap: Unwrap::default(),
                };
                (format!("cpu-{name}"), source)
            })
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Returns the hwmon energy counters of a device directory.
fn hwmon_domains(dir: &Path) -> Vec<(String, Source)> {
    let Ok(entries) = fs::read_dir(dir.join("hwmon")) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path().join("energy1_input"))
        .filter(|p| p.exists())
        .collect();
    paths.sort();
    paths
        .into_iter()
        .enumerate()
        .map(|(i, p)| (format!("gpu-hwmon-{i}"), Source::Hwmon(p)))
        .collect()
}

fn read_number(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_at_the_range() {
        let range = 262_143_328_850;
        let mut u = Unwrap::default();
        assert_eq!(u.update(262_143_000_000, range), 262_143_000_000);
        assert_eq!(u.update(262_143_300_000, range), 262_143_300_000);
        // The counter wraps: 28 850 to the range, then 1 000 past zero.
        assert_eq!(u.update(1_000, range), 262_143_329_850);
        assert_eq!(u.update(6_000, range), 262_143_334_850);
    }

    #[test]
    fn finds_package_zones_and_hwmon_counters() {
        let root = std::env::temp_dir().join(format!("fakeland-energy-{}", std::process::id()));
        let zone = |name: &str, label: &str, uj: &str| {
            let d = root.join("powercap").join(name);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("name"), format!("{label}\n")).unwrap();
            fs::write(d.join("energy_uj"), uj).unwrap();
            fs::write(d.join("max_energy_range_uj"), "1000\n").unwrap();
        };
        zone("intel-rapl:0", "package-0", "900\n");
        zone("intel-rapl:0:0", "core", "5\n");
        zone("intel-rapl:1", "psys", "7\n");
        let hw = root.join("device/hwmon/hwmon3");
        fs::create_dir_all(&hw).unwrap();
        fs::write(hw.join("energy1_input"), "123456\n").unwrap();
        let mut domains = powercap_domains(&root.join("powercap"));
        domains.extend(hwmon_domains(&root.join("device")));
        let mut meter = EnergyMeter { domains };
        assert_eq!(meter.names(), ["cpu-package-0", "gpu-hwmon-0"]);
        assert_eq!(meter.read(false)[1].microjoules_total, 123456);
        fs::write(root.join("powercap/intel-rapl:0/energy_uj"), "100\n").unwrap();
        let r = meter.read(false);
        assert_eq!(r[0].domain, "cpu-package-0");
        assert_eq!(r[0].microjoules_total, 1100);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn opens_without_devices() {
        let meter = EnergyMeter::open(&GpuContext::default());
        assert!(meter.names().iter().all(|n| n.starts_with("cpu-")));
    }
}
