//! The config of a new output from its text form, with a free connector name.

use fakeland::OutputConfig;

use super::{SpecError, parse_output};

/// Builds a new output from `[NAME:]WIDTHxHEIGHT[@HZ]`. Without a name the output takes the first
/// `DP-N` outside `taken`.
pub fn new_output(spec: &str, taken: &[String]) -> Result<OutputConfig, SpecError> {
    let (name, mode) = parse_output(spec)?;
    let name = name.unwrap_or_else(|| {
        (1..)
            .map(|i| format!("DP-{i}"))
            .find(|n| !taken.contains(n))
            .unwrap_or_default()
    });
    Ok(OutputConfig::new(name).mode(mode.width, mode.height, mode.refresh))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_new_outputs() {
        let taken = ["DP-1".to_owned(), "DP-3".to_owned()];
        let c = new_output("1280x1024@75", &taken).unwrap();
        assert_eq!(c.name, "DP-2");
        assert_eq!((c.mode.width, c.mode.height), (1280, 1024));
        assert_eq!(c.mode.refresh.0, 75.0);
        assert_eq!(c.position, None);
        assert_eq!(
            new_output("HDMI-A-1:800x600", &taken).unwrap().name,
            "HDMI-A-1"
        );
        assert!(new_output("big", &taken).is_err());
    }
}
