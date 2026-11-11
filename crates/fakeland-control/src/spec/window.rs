//! The text form of a window matcher.

use fakeland_scenario::WindowRef;
use fakeland_types::WindowId;

use super::SpecError;

/// Parses a window matcher: `focused`, `any`, a window id such as `3` or `id:3`, `app:APP_ID`
/// or `title:REGEX`. Any other text matches the app id.
pub fn parse_window_ref(s: &str) -> Result<WindowRef, SpecError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(SpecError::Window(s.to_owned()));
    }
    let id = |v: &str| {
        v.trim_start_matches("window#")
            .parse::<u64>()
            .map(|n| WindowRef::id(WindowId(n)))
            .map_err(|_| SpecError::Window(s.to_owned()))
    };
    Ok(match s.split_once(':') {
        Some(("id", v)) => id(v)?,
        Some(("app" | "app_id" | "app-id", v)) => WindowRef::app_id(v),
        Some(("title", v)) => WindowRef::title_regex(v),
        _ if s == "focused" => WindowRef::Focused,
        _ if s == "any" => WindowRef::Any,
        _ if s
            .trim_start_matches("window#")
            .chars()
            .all(|c| c.is_ascii_digit()) =>
        {
            id(s)?
        }
        _ => WindowRef::app_id(s),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_matchers() {
        assert_eq!(parse_window_ref("focused").unwrap(), WindowRef::Focused);
        assert_eq!(parse_window_ref("any").unwrap(), WindowRef::Any);
        assert_eq!(parse_window_ref("3").unwrap(), WindowRef::id(WindowId(3)));
        assert_eq!(
            parse_window_ref("id:4").unwrap(),
            WindowRef::id(WindowId(4))
        );
        assert_eq!(
            parse_window_ref("app:org.gnome.Zenity").unwrap(),
            WindowRef::app_id("org.gnome.Zenity")
        );
        assert_eq!(
            parse_window_ref("title:^Hello").unwrap(),
            WindowRef::title_regex("^Hello")
        );
        assert_eq!(
            parse_window_ref("Alacritty").unwrap(),
            WindowRef::app_id("Alacritty")
        );
        assert!(parse_window_ref("id:x").is_err());
        assert!(parse_window_ref(" ").is_err());
    }
}
