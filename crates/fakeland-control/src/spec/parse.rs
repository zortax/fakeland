//! Parsers of the setting text forms.

use fakeland::{Composite, ModeConfig, RendererChoice, WindowPolicy};
use fakeland_timing::{CallbackPolicy, TimingConfig};
use fakeland_types::{Dur, Hz, Logical, Size};

use super::SpecError;

/// Parses `WIDTHxHEIGHT`.
pub fn parse_size(s: &str) -> Result<(i32, i32), SpecError> {
    let err = || SpecError::Size(s.to_owned());
    let (w, h) = s.trim().split_once(['x', 'X']).ok_or_else(err)?;
    let w: i32 = w.trim().parse().map_err(|_| err())?;
    let h: i32 = h.trim().parse().map_err(|_| err())?;
    if w <= 0 || h <= 0 {
        return Err(err());
    }
    Ok((w, h))
}

fn parse_hz(s: &str) -> Option<Hz> {
    let s = s.trim();
    let s = s
        .strip_suffix("Hz")
        .or_else(|| s.strip_suffix("hz"))
        .unwrap_or(s);
    let hz: f64 = s.trim().parse().ok()?;
    (hz.is_finite() && hz > 0.0).then_some(Hz(hz))
}

/// Parses `[NAME:]WIDTHxHEIGHT[@HZ]`. The refresh rate defaults to 60 Hz.
pub fn parse_output(s: &str) -> Result<(Option<String>, ModeConfig), SpecError> {
    let err = || SpecError::Output(s.to_owned());
    let (name, rest) = match s.rsplit_once(':') {
        Some((n, r)) if !n.is_empty() => (Some(n.to_owned()), r),
        _ => (None, s),
    };
    let (size, hz) = match rest.split_once('@') {
        Some((size, hz)) => (size, parse_hz(hz).ok_or_else(err)?),
        None => (rest, Hz(60.0)),
    };
    let (w, h) = parse_size(size).map_err(|_| err())?;
    Ok((name, ModeConfig::new(w, h, hz)))
}

/// Parses a timing model. `refresh` is the mode refresh of the output: fixed timing follows
/// it, and manual timing takes it as the nominal rate unless the form names one.
pub fn parse_timing(s: &str, refresh: Hz) -> Result<TimingConfig, SpecError> {
    let err = || SpecError::Timing(s.to_owned());
    let (kind, arg) = match s.trim().split_once(':') {
        Some((k, a)) => (k, Some(a)),
        None => (s.trim(), None),
    };
    match (kind, arg) {
        ("fixed", None) => Ok(TimingConfig::fixed(refresh)),
        ("fixed", Some(hz)) => Ok(TimingConfig::fixed(parse_hz(hz).ok_or_else(err)?)),
        ("vrr", Some(range)) => {
            let (lo, hi) = range.split_once('-').ok_or_else(err)?;
            let (lo, hi) = (parse_hz(lo).ok_or_else(err)?, parse_hz(hi).ok_or_else(err)?);
            Ok(TimingConfig::vrr(lo, hi))
        }
        ("vrr", None) => Ok(TimingConfig::vrr(Hz(48.0), refresh)),
        ("unthrottled", None) => Ok(TimingConfig::unthrottled()),
        ("manual", None) => Ok(TimingConfig::manual(refresh)),
        ("manual", Some(hz)) => Ok(TimingConfig::manual(parse_hz(hz).ok_or_else(err)?)),
        _ => Err(err()),
    }
}

fn norm(s: &str) -> String {
    s.trim().to_ascii_lowercase().replace('_', "-")
}

/// Parses `at-vblank`, `before-vblank:LEAD` or `on-present`.
pub fn parse_callbacks(s: &str) -> Result<CallbackPolicy, SpecError> {
    let n = norm(s);
    let (kind, arg) = match n.split_once(':') {
        Some((k, a)) => (k.to_owned(), Some(a.to_owned())),
        None => (n.clone(), None),
    };
    match (kind.as_str(), arg) {
        ("at-vblank", None) => Ok(CallbackPolicy::AtVblank),
        ("on-present", None) => Ok(CallbackPolicy::OnPresent),
        ("before-vblank", lead) => {
            let lead = match lead {
                Some(l) => l
                    .parse::<Dur>()
                    .map_err(|_| SpecError::Callbacks(s.to_owned()))?,
                None => Dur::from_millis(3),
            };
            Ok(CallbackPolicy::BeforeVblank { lead: lead.0 })
        }
        _ => Err(SpecError::Callbacks(s.to_owned())),
    }
}

/// Parses `auto`, `gles` or `pixman`.
pub fn parse_renderer(s: &str) -> Result<RendererChoice, SpecError> {
    match norm(s).as_str() {
        "auto" => Ok(RendererChoice::Auto),
        "gles" | "gl" | "gpu" => Ok(RendererChoice::Gles),
        "pixman" | "cpu" => Ok(RendererChoice::Pixman),
        _ => Err(SpecError::Renderer(s.to_owned())),
    }
}

/// Parses `lazy` or `every-frame`.
pub fn parse_composite(s: &str) -> Result<Composite, SpecError> {
    match norm(s).as_str() {
        "lazy" => Ok(Composite::Lazy),
        "every-frame" => Ok(Composite::EveryFrame),
        _ => Err(SpecError::Composite(s.to_owned())),
    }
}

/// Parses `kiosk`, `floating`, `floating:WxH` or `fullscreen`.
pub fn parse_window_policy(s: &str) -> Result<WindowPolicy, SpecError> {
    let n = norm(s);
    let err = || SpecError::WindowPolicy(s.to_owned());
    match n.split_once(':') {
        None if n == "kiosk" => Ok(WindowPolicy::Kiosk),
        None if n == "fullscreen" => Ok(WindowPolicy::Fullscreen),
        None if n == "floating" => Ok(WindowPolicy::Floating {
            default_size: Size::<Logical>::new(800, 600),
        }),
        Some(("floating", size)) => {
            let (w, h) = parse_size(size).map_err(|_| err())?;
            Ok(WindowPolicy::Floating {
                default_size: Size::new(w, h),
            })
        }
        _ => Err(err()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_timing::TimingMode;
    use fakeland_types::Nanos;

    #[test]
    fn parses_outputs() {
        let (name, mode) = parse_output("2560x1440@143.912").unwrap();
        assert_eq!(name, None);
        assert_eq!((mode.width, mode.height), (2560, 1440));
        assert!((mode.refresh.0 - 143.912).abs() < 1e-9);
        let (name, mode) = parse_output("HDMI-A-1:1280x720").unwrap();
        assert_eq!(name.as_deref(), Some("HDMI-A-1"));
        assert_eq!(mode.refresh, Hz(60.0));
        assert!(parse_output("1280x").is_err());
        assert!(parse_output("1280x720@0").is_err());
        assert!(parse_output("0x720").is_err());
    }

    #[test]
    fn parses_timing() {
        let t = parse_timing("fixed", Hz(144.0)).unwrap();
        assert_eq!(t.mode, TimingMode::Fixed { refresh: Hz(144.0) });
        let t = parse_timing("vrr:48-144", Hz(60.0)).unwrap();
        assert_eq!(
            t.mode,
            TimingMode::Vrr {
                min: Hz(48.0),
                max: Hz(144.0)
            }
        );
        assert_eq!(
            parse_timing("unthrottled", Hz(60.0)).unwrap().mode,
            TimingMode::Unthrottled
        );
        assert_eq!(
            parse_timing("manual", Hz(75.0)).unwrap().mode,
            TimingMode::Manual { nominal: Hz(75.0) }
        );
        assert!(parse_timing("vrr:48", Hz(60.0)).is_err());
        assert!(parse_timing("warp", Hz(60.0)).is_err());
    }

    #[test]
    fn parses_callbacks() {
        assert_eq!(
            parse_callbacks("at-vblank").unwrap(),
            CallbackPolicy::AtVblank
        );
        assert_eq!(
            parse_callbacks("on_present").unwrap(),
            CallbackPolicy::OnPresent
        );
        assert_eq!(
            parse_callbacks("before-vblank:3ms").unwrap(),
            CallbackPolicy::BeforeVblank {
                lead: Nanos::from_millis(3)
            }
        );
        assert!(parse_callbacks("before-vblank:soon").is_err());
    }

    #[test]
    fn parses_policies() {
        assert_eq!(parse_renderer("pixman").unwrap(), RendererChoice::Pixman);
        assert_eq!(
            parse_composite("every_frame").unwrap(),
            Composite::EveryFrame
        );
        assert_eq!(parse_window_policy("kiosk").unwrap(), WindowPolicy::Kiosk);
        assert_eq!(
            parse_window_policy("floating:640x480").unwrap(),
            WindowPolicy::Floating {
                default_size: Size::new(640, 480)
            }
        );
        assert!(parse_window_policy("tiled").is_err());
    }
}
