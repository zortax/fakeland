//! GPU samples from driver counters in sysfs: `gpu_busy_percent` and the VRAM counters that
//! amdgpu publishes in the device directory.

use std::fs;
use std::path::{Path, PathBuf};

use super::{GpuProbe, GpuReading};

/// Reads the sysfs counters of one device.
#[derive(Debug)]
pub struct SysfsProbe {
    dir: PathBuf,
}

impl SysfsProbe {
    /// Opens the counters of a device directory, or `None` when the driver has no busy counter.
    pub fn open(dir: &Path) -> Option<Self> {
        let probe = Self {
            dir: dir.to_path_buf(),
        };
        probe.busy()?;
        Some(probe)
    }

    fn busy(&self) -> Option<f64> {
        read_number(&self.dir.join("gpu_busy_percent")).map(|v| v as f64)
    }
}

impl GpuProbe for SysfsProbe {
    fn name(&self) -> &'static str {
        "sysfs"
    }

    fn read(&mut self) -> Option<GpuReading> {
        Some(GpuReading {
            util_percent: self.busy()?,
            mem_used_bytes: read_number(&self.dir.join("mem_info_vram_used")).unwrap_or(0),
            mem_total_bytes: read_number(&self.dir.join("mem_info_vram_total")).unwrap_or(0),
        })
    }
}

fn read_number(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_counters_from_a_directory() {
        let dir = std::env::temp_dir().join(format!("fakeland-sysfs-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(SysfsProbe::open(&dir).is_none());
        fs::write(dir.join("gpu_busy_percent"), "37\n").unwrap();
        fs::write(dir.join("mem_info_vram_used"), "1048576\n").unwrap();
        fs::write(dir.join("mem_info_vram_total"), "8589934592\n").unwrap();
        let mut probe = SysfsProbe::open(&dir).unwrap();
        assert_eq!(
            probe.read(),
            Some(GpuReading {
                util_percent: 37.0,
                mem_used_bytes: 1 << 20,
                mem_total_bytes: 8 << 30,
            })
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
