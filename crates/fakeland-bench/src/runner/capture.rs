//! Points frame captures of the focused window at the target window.

use fakeland::{CaptureSpec, WindowInfo};
use fakeland_scenario::WindowRef;

/// Points a frame capture of the focused window at the target.
pub(crate) fn resolve(spec: &CaptureSpec, window: Option<&WindowInfo>) -> CaptureSpec {
    let Some(w) = window else {
        return spec.clone();
    };
    if spec.window() != &WindowRef::Focused {
        return spec.clone();
    }
    let me = WindowRef::id(w.id);
    match spec.clone() {
        CaptureSpec::EveryCommit {
            roi,
            downscale,
            max_frames,
            ..
        } => CaptureSpec::EveryCommit {
            window: me,
            roi,
            downscale,
            max_frames,
        },
        CaptureSpec::EveryPresented {
            roi,
            downscale,
            max_frames,
            ..
        } => CaptureSpec::EveryPresented {
            window: me,
            roi,
            downscale,
            max_frames,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland::Placement;
    use fakeland_types::{OutputId, Rect, SurfaceId, WindowId};

    #[test]
    fn focused_captures_follow_the_target() {
        let w = WindowInfo {
            id: WindowId(7),
            app_id: "a".into(),
            title: String::new(),
            pid: None,
            app: None,
            output: OutputId(1),
            geometry: Rect::new(0, 0, 10, 10),
            surface: SurfaceId(3),
            mapped_at: None,
            focused: true,
            suspended: false,
            placement: Placement::Maximized,
        };
        let spec = CaptureSpec::every_presented(WindowRef::Focused)
            .with_downscale(2)
            .with_roi(Rect::new(1, 2, 3, 4));
        let r = resolve(&spec, Some(&w));
        assert_eq!(r.window(), &WindowRef::id(WindowId(7)));
        assert_eq!((r.downscale(), r.roi()), (2, Some(Rect::new(1, 2, 3, 4))));
        let other = CaptureSpec::every_commit(WindowRef::app_id("x"));
        assert_eq!(resolve(&other, Some(&w)), other);
        assert_eq!(resolve(&spec, None), spec);
    }
}
