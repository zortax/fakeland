//! Screenshot and wait tools.

use anyhow::{Context, Result};
use fakeland_scenario::{Roi, Wait, WindowRef};
use fakeland_types::{Dur, Rect};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{tool, tool_router};

use super::common::{limit, matcher, ms, window_line};
use crate::params::{Region, ScreenshotParams, WaitForParams, WaitKind};
use crate::reply::{ToolResult, done, finish};
use crate::server::FakelandMcp;
use crate::state::bounded;

/// Builds the region of a pixel wait.
pub fn roi(window: Option<WindowRef>, region: Option<Region>) -> Roi {
    let rect = region.map(|r| Rect::new(r.x, r.y, r.w, r.h));
    match (window, rect) {
        (Some(w), Some(r)) => Roi::window_rect(w, r),
        (Some(w), None) => Roi::window(w),
        (None, Some(r)) => Roi::output_rect(None, r),
        (None, None) => Roi::output(None),
    }
}

fn wait_of(p: &WaitForParams) -> Result<Wait> {
    let window = matcher(p.window.as_deref())?;
    let wait = match p.until {
        WaitKind::Window => Wait::window(window.unwrap_or(WindowRef::Any)),
        WaitKind::Settled => {
            Wait::settled(Some(window.unwrap_or(WindowRef::Any)), Dur(ms(p.quiet_ms)))
        }
        WaitKind::Frames => Wait::frames(p.frames.max(1), window),
        WaitKind::PixelsStable => Wait::pixels_stable(roi(window, p.region), p.frames.max(1)),
        WaitKind::PixelsMatch => Wait::pixels_match(
            roi(window, p.region),
            p.condition
                .clone()
                .context("pixels_match needs a condition")?,
        ),
    };
    Ok(wait.with_timeout(Dur(ms(p.timeout_ms.max(1)))))
}

#[tool_router(router = screen_router, vis = "pub(crate)")]
impl FakelandMcp {
    /// Takes a screenshot.
    #[tool(
        description = "Take a screenshot of the primary output, an output, a window, or a logical region. Returns a PNG image downscaled to max_dim (default 1280) and its coordinate scale. Pointer tools then take pixel coordinates of this image. By default it first waits up to 1 s for the content to settle."
    )]
    async fn screenshot(&self, Parameters(p): Parameters<ScreenshotParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                Ok(CallToolResult::success(self.shoot(&c, &p).await?))
            }
            .await,
        )
    }

    /// Waits on the compositor.
    #[tool(
        description = "Wait on the compositor until: window (a window maps), settled (no new frame for quiet_ms), frames (n presented frames), pixels_stable (region unchanged for frames refreshes), or pixels_match (region meets condition). Fails after timeout_ms."
    )]
    async fn wait_for(&self, Parameters(p): Parameters<WaitForParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let wait = wait_of(&p)?;
                let out = bounded(limit(p.timeout_ms), c.wait(wait)).await??;
                let window = out
                    .window
                    .as_ref()
                    .map_or_else(String::new, |w| format!(": {}", window_line(w)));
                done(format!("Done after the wait{window}"))
            }
            .await,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_scenario::PixelCondition;

    fn params(until: WaitKind) -> WaitForParams {
        WaitForParams {
            until,
            window: None,
            timeout_ms: 500,
            quiet_ms: 100,
            frames: 2,
            region: Some(Region {
                x: 1,
                y: 2,
                w: 3,
                h: 4,
            }),
            condition: None,
        }
    }

    #[test]
    fn builds_waits() {
        let w = wait_of(&params(WaitKind::PixelsStable)).unwrap();
        assert_eq!(w.timeout(), Dur::from_millis(500));
        assert!(matches!(w, Wait::PixelsStable { frames: 2, .. }));
        assert!(wait_of(&params(WaitKind::PixelsMatch)).is_err());
        let mut p = params(WaitKind::PixelsMatch);
        p.condition = Some(PixelCondition::Changed { threshold: 0.01 });
        assert!(wait_of(&p).is_ok());
        assert!(matches!(
            wait_of(&params(WaitKind::Window)).unwrap(),
            Wait::Window { .. }
        ));
    }
}
