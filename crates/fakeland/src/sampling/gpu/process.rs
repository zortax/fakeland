//! GPU use of each process tree: memory, engine busy time and utilization. Backends run in order,
//! and a tree takes the reading of the first backend that has data for it: the DRM fdinfo of the
//! tree members, then NVML with the `nvml` feature.

use fakeland_types::Timestamp;

use super::GpuContext;

/// The members of one tree.
#[derive(Clone, Copy, Debug)]
pub struct TreeMembers<'a> {
    /// The root pid.
    pub root: u32,
    /// The member pids.
    pub pids: &'a [u32],
}

/// The GPU use of one tree at one sample.
#[derive(Clone, Debug, PartialEq)]
pub struct GpuProcessReading {
    /// The root pid.
    pub root: u32,
    /// GPU memory of every member in bytes.
    pub mem_bytes: Option<u64>,
    /// The engine busy time of every member since the first sample, in nanoseconds.
    pub busy_ns_total: Option<u64>,
    /// GPU utilization of every member in percent.
    pub util_percent: Option<f64>,
    /// The source, for example `nvml` or `fdinfo:amdgpu`.
    pub source: String,
}

impl GpuProcessReading {
    /// Returns true when the reading carries a value.
    pub fn has_data(&self) -> bool {
        self.mem_bytes.is_some() || self.busy_ns_total.is_some() || self.util_percent.is_some()
    }
}

/// A source of GPU readings per process tree.
pub trait GpuProcessProbe: Send + std::fmt::Debug {
    /// Returns the backend name, for example `fdinfo` or `nvml`.
    fn name(&self) -> &'static str;

    /// Reads the trees at time `t`. Returns a reading for each tree with data.
    fn read(&mut self, t: Timestamp, trees: &[TreeMembers<'_>]) -> Vec<GpuProcessReading>;

    /// Drops the state of a tree that ended.
    fn forget(&mut self, root: u32);
}

/// The backends in order.
#[derive(Debug, Default)]
pub struct GpuProcessChain {
    probes: Vec<Box<dyn GpuProcessProbe>>,
}

impl GpuProcessChain {
    /// Opens every backend that works on this host.
    pub fn open(ctx: &GpuContext) -> Self {
        let fdinfo: Box<dyn GpuProcessProbe> = Box::new(super::fdinfo::FdinfoProbe::default());
        #[cfg(feature = "nvml")]
        let nvml = ctx
            .nvml
            .clone()
            .and_then(super::nvml::NvmlProcessProbe::open)
            .map(|p| Box::new(p) as Box<dyn GpuProcessProbe>);
        #[cfg(not(feature = "nvml"))]
        let nvml = {
            let _ = ctx;
            None
        };
        Self {
            probes: std::iter::once(fdinfo).chain(nvml).collect(),
        }
    }

    /// Returns the backend names in order.
    pub fn names(&self) -> Vec<&'static str> {
        self.probes.iter().map(|p| p.name()).collect()
    }

    /// Reads every tree. Each backend reads every tree, so its counters stay current, and a tree
    /// keeps the reading of the first backend with data.
    pub fn read(&mut self, t: Timestamp, trees: &[TreeMembers<'_>]) -> Vec<GpuProcessReading> {
        let mut out: Vec<GpuProcessReading> = Vec::new();
        for probe in &mut self.probes {
            for r in probe.read(t, trees) {
                if r.has_data() && !out.iter().any(|o| o.root == r.root) {
                    out.push(r);
                }
            }
        }
        out.sort_by_key(|r| r.root);
        out
    }

    /// Drops the state of a tree that ended.
    pub fn forget(&mut self, root: u32) {
        for p in &mut self.probes {
            p.forget(root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Fixed(&'static str, Vec<GpuProcessReading>);

    impl GpuProcessProbe for Fixed {
        fn name(&self) -> &'static str {
            self.0
        }

        fn read(&mut self, _: Timestamp, _: &[TreeMembers<'_>]) -> Vec<GpuProcessReading> {
            self.1.clone()
        }

        fn forget(&mut self, _: u32) {}
    }

    fn reading(root: u32, mem: Option<u64>, source: &str) -> GpuProcessReading {
        GpuProcessReading {
            root,
            mem_bytes: mem,
            busy_ns_total: None,
            util_percent: None,
            source: source.into(),
        }
    }

    #[test]
    fn the_first_backend_with_data_wins_per_tree() {
        let mut chain = GpuProcessChain {
            probes: vec![
                Box::new(Fixed(
                    "fdinfo",
                    vec![
                        reading(1, Some(5), "fdinfo:i915"),
                        reading(2, None, "fdinfo:nv"),
                    ],
                )),
                Box::new(Fixed(
                    "nvml",
                    vec![reading(1, Some(9), "nvml"), reading(2, Some(7), "nvml")],
                )),
            ],
        };
        assert_eq!(chain.names(), ["fdinfo", "nvml"]);
        let out = chain.read(Timestamp(0), &[]);
        assert_eq!(
            out,
            [
                reading(1, Some(5), "fdinfo:i915"),
                reading(2, Some(7), "nvml")
            ]
        );
        chain.forget(1);
    }
}
