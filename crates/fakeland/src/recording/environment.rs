//! Detection of the host environment for the trace header: GPU, driver, kernel, CPU, governor,
//! renderer and hostname. Unknown values stay empty.

use std::fs;
use std::path::Path;

use fakeland_trace::Environment;

/// Facts about the render device that the renderer knows.
#[derive(Debug, Clone, Default)]
pub struct DeviceFacts {
    /// The render node path, for example `/dev/dri/renderD128`.
    pub node: Option<String>,
    /// The DRM driver name, for example `nvidia-drm` or `amdgpu`.
    pub driver: Option<String>,
    /// The renderer name: `gles` or `pixman`.
    pub renderer: String,
}

/// Reads the environment of this host.
pub fn detect(device: &DeviceFacts) -> Environment {
    Environment {
        gpu: device
            .node
            .as_deref()
            .and_then(gpu_name)
            .unwrap_or_default(),
        driver: driver_string(device.driver.as_deref()),
        kernel: read_trimmed("/proc/sys/kernel/osrelease").unwrap_or_default(),
        cpu: cpu_model().unwrap_or_default(),
        governor: read_trimmed("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
            .unwrap_or_default(),
        renderer: device.renderer.clone(),
        hostname: read_trimmed("/proc/sys/kernel/hostname").unwrap_or_default(),
        fakeland_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    let s = fs::read_to_string(path).ok()?;
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Returns the CPU model from `/proc/cpuinfo`.
fn cpu_model() -> Option<String> {
    parse_cpu_model(&fs::read_to_string("/proc/cpuinfo").ok()?)
}

/// Returns the first `model name` value of a cpuinfo text.
pub fn parse_cpu_model(cpuinfo: &str) -> Option<String> {
    cpuinfo.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == "model name").then(|| v.trim().to_string())
    })
}

/// Returns the GPU name of a render node. The NVIDIA driver names the model in procfs; other
/// drivers give the PCI vendor and device ids.
fn gpu_name(node: &str) -> Option<String> {
    let name = Path::new(node).file_name()?.to_str()?;
    let device = Path::new("/sys/class/drm").join(name).join("device");
    let vendor = read_trimmed(device.join("vendor"));
    let product = read_trimmed(device.join("device"));
    if vendor.as_deref() == Some("0x10de")
        && let Some(model) = nvidia_model(&device)
    {
        return Some(model);
    }
    match (vendor, product) {
        (Some(v), Some(d)) => Some(format!("pci {v}:{d}")),
        _ => None,
    }
}

/// Reads the NVIDIA model name of the GPU at a sysfs device path.
fn nvidia_model(device: &Path) -> Option<String> {
    let real = fs::canonicalize(device).ok()?;
    let bus = real.file_name()?.to_str()?.to_lowercase();
    for entry in fs::read_dir("/proc/driver/nvidia/gpus").ok()?.flatten() {
        let dir_name = entry.file_name().to_string_lossy().to_lowercase();
        if !bus.ends_with(dir_name.trim_start_matches("0000:")) && dir_name != bus {
            continue;
        }
        let info = fs::read_to_string(entry.path().join("information")).ok()?;
        return parse_nvidia_model(&info);
    }
    None
}

/// Returns the `Model:` value of an NVIDIA gpu information file.
pub fn parse_nvidia_model(info: &str) -> Option<String> {
    info.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        (k.trim() == "Model").then(|| v.trim().to_string())
    })
}

/// Returns the driver name with the loaded module version when known.
fn driver_string(driver: Option<&str>) -> String {
    let Some(driver) = driver else {
        return String::new();
    };
    let module = match driver {
        "nvidia-drm" => "nvidia",
        other => other,
    };
    match read_trimmed(format!("/sys/module/{module}/version")) {
        Some(version) => format!("{driver} {version}"),
        None => driver.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cpuinfo_and_nvidia_info() {
        let cpuinfo = "processor\t: 0\nvendor_id\t: AuthenticAMD\nmodel name\t: AMD Ryzen 9 5950X 16-Core Processor\n";
        assert_eq!(
            parse_cpu_model(cpuinfo).as_deref(),
            Some("AMD Ryzen 9 5950X 16-Core Processor")
        );
        let info = "Model: \t\t NVIDIA GeForce RTX 3080 Ti\nIRQ:   123\n";
        assert_eq!(
            parse_nvidia_model(info).as_deref(),
            Some("NVIDIA GeForce RTX 3080 Ti")
        );
    }

    #[test]
    fn detect_fills_host_facts() {
        let env = detect(&DeviceFacts {
            renderer: "pixman".into(),
            ..DeviceFacts::default()
        });
        assert!(!env.kernel.is_empty());
        assert_eq!(env.renderer, "pixman");
        assert!(!env.fakeland_version.is_empty());
    }
}
