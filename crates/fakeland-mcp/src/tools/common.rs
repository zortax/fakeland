//! Helpers shared by the tools: the session, screenshots as image blocks, and screenshots after
//! actions.

use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use fakeland::{CaptureOptions, WindowInfo};
use fakeland_control::Controller;
use fakeland_control::methods::NoParams;
use fakeland_control::ops::encode_png;
use fakeland_control::text::windows_text;
use fakeland_scenario::{CaptureTarget, Roi, WindowRef};
use fakeland_types::{Nanos, OutputId, Rect};
use rmcp::model::{CallToolResult, ContentBlock};

use crate::coords::ShotFrame;
use crate::params::ScreenshotParams;
use crate::server::FakelandMcp;
use crate::state::{CALL_SLACK, bounded};

/// The settle time of the screenshot after an action.
const AFTER_SETTLE_MS: u64 = 150;

/// Returns the window matcher of a tool parameter.
pub fn matcher(window: Option<&str>) -> Result<Option<WindowRef>> {
    Ok(window
        .map(fakeland_control::spec::parse_window_ref)
        .transpose()?)
}

/// Returns a duration in milliseconds as nanoseconds.
pub fn ms(v: u64) -> Nanos {
    Nanos::from_millis(i64::try_from(v).unwrap_or(i64::MAX / 1_000_000))
}

/// Returns a timeout plus slack as a host duration for [`bounded`].
pub fn limit(ms: u64) -> Duration {
    Duration::from_millis(ms) + CALL_SLACK
}

impl FakelandMcp {
    /// Returns the session, started on first use.
    pub(crate) async fn ctl(&self) -> Result<Controller> {
        self.state.controller().await
    }

    /// Returns the windows as text.
    pub(crate) async fn windows_line(&self, c: &Controller) -> String {
        match bounded(CALL_SLACK, c.windows(NoParams {})).await {
            Ok(Ok(w)) => format!("Windows:\n{}", windows_text(&w)),
            _ => String::new(),
        }
    }

    /// Takes a screenshot, records its pixel frame and returns the image and its description.
    pub(crate) async fn shoot(
        &self,
        c: &Controller,
        p: &ScreenshotParams,
    ) -> Result<Vec<ContentBlock>> {
        let window = matcher(p.window.as_deref())?;
        if p.settle_ms > 0 {
            let quiet = ms(p.settle_ms);
            let timeout = ms(p.settle_ms.saturating_mul(4).clamp(250, 1000));
            let target = window.clone().unwrap_or(WindowRef::Any);
            let _ = bounded(
                limit(1000),
                c.session().wait_settled(target, quiet, timeout),
            )
            .await;
        }
        let output = p.output.map(OutputId);
        let rect = p.region.map(|r| Rect::new(r.x, r.y, r.w, r.h));
        let target = match (window, rect) {
            (Some(w), None) => CaptureTarget::Window { window: w },
            (Some(w), Some(r)) => CaptureTarget::Roi {
                roi: Roi::window_rect(w, r),
            },
            (None, Some(r)) => CaptureTarget::Roi {
                roi: Roi::Output {
                    output,
                    rect: Some(r),
                },
            },
            (None, None) => CaptureTarget::Output { output },
        };
        let options = CaptureOptions {
            include_cursor: p.cursor,
            max_dim: (p.max_dim > 0).then_some(p.max_dim),
        };
        let shot = bounded(CALL_SLACK, c.session().screenshot(target, options)).await??;
        let png = encode_png(&shot)?;
        let frame = ShotFrame {
            region: shot.region,
            scale: shot.scale,
            output: shot.output,
            window: shot.window,
            size: (shot.image.width(), shot.image.height()),
        };
        self.state.set_shot(frame);
        let r = shot.region;
        let what = match (shot.window, shot.output) {
            (Some(w), _) => format!("{w}"),
            (None, Some(o)) => format!("{o}"),
            (None, None) => "the output".to_owned(),
        };
        let text = format!(
            "Screenshot of {what}: {}x{} px. 1 px = {:.4} logical px. It shows the logical \
             rectangle x={} y={} w={} h={} of the global space. Pointer tools take pixel \
             coordinates of this image.\n{}",
            frame.size.0,
            frame.size.1,
            shot.scale,
            r.x,
            r.y,
            r.w,
            r.h,
            self.windows_line(c).await
        );
        Ok(vec![
            ContentBlock::image(
                base64::engine::general_purpose::STANDARD.encode(png),
                "image/png",
            ),
            ContentBlock::text(text),
        ])
    }

    /// Finishes an action: its summary, and a screenshot of the same target as the last one
    /// when asked.
    pub(crate) async fn after(
        &self,
        c: &Controller,
        screenshot: bool,
        summary: String,
    ) -> Result<CallToolResult> {
        let mut content = vec![ContentBlock::text(summary)];
        if screenshot {
            let last = self.state.shot();
            let p = ScreenshotParams {
                window: last.and_then(|f| f.window).map(|w| w.0.to_string()),
                output: last.and_then(|f| f.output).map(|o| o.0),
                region: None,
                max_dim: last.map_or(1280, |f| f.size.0.max(f.size.1)),
                cursor: false,
                settle_ms: AFTER_SETTLE_MS,
            };
            content.extend(self.shoot(c, &p).await?);
        }
        Ok(CallToolResult::success(content))
    }
}

/// Describes a window in one line.
pub fn window_line(w: &WindowInfo) -> String {
    windows_text(std::slice::from_ref(w)).trim_end().to_owned()
}
