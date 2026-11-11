//! The title of a viewer window: the output name and mode, and the name and frame rate of the
//! focused window when it shows on that output.

/// What a title shows.
#[derive(Clone, Debug, PartialEq)]
pub struct TitleInfo<'a> {
    /// The output name.
    pub output: &'a str,
    /// The mode width.
    pub width: i32,
    /// The mode height.
    pub height: i32,
    /// The mode refresh in Hz.
    pub refresh: f64,
    /// The output scale.
    pub scale: f64,
    /// The app id, or the title without one, and the frame rate of the focused window on this
    /// output.
    pub focused: Option<(&'a str, f64)>,
}

/// Builds the window title.
pub fn title(info: &TitleInfo<'_>) -> String {
    let mut s = format!(
        "fakeland {} {}x{}@{:.2}",
        info.output, info.width, info.height, info.refresh
    );
    if (info.scale - 1.0).abs() > 1e-9 {
        s.push_str(&format!(" x{}", info.scale));
    }
    if let Some((app, fps)) = info.focused {
        let app = if app.is_empty() { "window" } else { app };
        s.push_str(&format!(" | {app} {fps:.1} fps"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_mode_scale_and_focused_fps() {
        let mut info = TitleInfo {
            output: "DP-1",
            width: 2560,
            height: 1440,
            refresh: 143.912,
            scale: 1.5,
            focused: Some(("vkcube", 143.84)),
        };
        assert_eq!(
            title(&info),
            "fakeland DP-1 2560x1440@143.91 x1.5 | vkcube 143.8 fps"
        );
        info.scale = 1.0;
        info.focused = Some(("", 0.0));
        assert_eq!(
            title(&info),
            "fakeland DP-1 2560x1440@143.91 | window 0.0 fps"
        );
        info.focused = None;
        assert_eq!(title(&info), "fakeland DP-1 2560x1440@143.91");
    }
}
