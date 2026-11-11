//! The sysfs view of the render device: its device directory, PCI vendor and PCI address.

use std::fs;
use std::path::{Path, PathBuf};

/// The PCI vendor id of NVIDIA.
pub const NVIDIA: &str = "0x10de";

/// The render device as sysfs sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderDevice {
    /// The device directory, for example `/sys/class/drm/renderD128/device`.
    pub dir: PathBuf,
    /// The PCI vendor id, for example `0x10de`.
    pub vendor: String,
    /// The PCI address, for example `0000:01:00.0`.
    pub pci: Option<String>,
}

impl RenderDevice {
    /// Finds the device of a render node path such as `/dev/dri/renderD128`.
    pub fn of_node(node: &Path) -> Option<Self> {
        let name = node.file_name()?;
        Self::at(&Path::new("/sys/class/drm").join(name).join("device"))
    }

    /// Reads the device at a sysfs device directory.
    pub fn at(dir: &Path) -> Option<Self> {
        let vendor = fs::read_to_string(dir.join("vendor")).ok()?;
        let pci = fs::canonicalize(dir)
            .ok()
            .and_then(|real| real.file_name()?.to_str().map(str::to_string))
            .filter(|s| is_pci_address(s));
        Some(Self {
            dir: dir.to_path_buf(),
            vendor: vendor.trim().to_string(),
            pci,
        })
    }

    /// Returns true for an NVIDIA GPU.
    pub fn is_nvidia(&self) -> bool {
        self.vendor == NVIDIA
    }
}

/// Returns true for a PCI address of the form `dddd:bb:dd.f`.
pub fn is_pci_address(s: &str) -> bool {
    let parts: Vec<&str> = s.split([':', '.']).collect();
    parts.len() == 4
        && [4, 2, 2, 1]
            .iter()
            .zip(&parts)
            .all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Returns true when two PCI bus ids name the same device. NVML pads the domain to eight digits.
#[cfg(any(feature = "nvml", test))]
pub fn same_pci(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let s = s.to_ascii_lowercase();
        match s.split_once(':') {
            Some((domain, rest)) => format!("{:0>8}:{rest}", domain.trim_start_matches('0')),
            None => s,
        }
    };
    norm(a) == norm(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_pci_addresses() {
        assert!(is_pci_address("0000:01:00.0"));
        assert!(!is_pci_address("card0"));
        assert!(!is_pci_address("0000:01:00"));
    }

    #[test]
    fn compares_padded_bus_ids() {
        assert!(same_pci("0000:01:00.0", "00000000:01:00.0"));
        assert!(same_pci("0000:0A:00.0", "00000000:0a:00.0"));
        assert!(!same_pci("0000:01:00.0", "00000000:02:00.0"));
    }

    #[test]
    fn missing_device_reads_as_none() {
        assert_eq!(RenderDevice::at(Path::new("/nonexistent/device")), None);
    }
}
