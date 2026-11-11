//! Pointer and keyboard tools.

use anyhow::{Context, Result, bail};
use fakeland::SentInput;
use fakeland_control::methods::{
    ClickParams as CtlClick, ComboParams, ScrollParams as CtlScroll, TypeTextParams as CtlType,
};
use fakeland_scenario::{Action, Button, Drag, Path, ScrollSource, TextVia, TypingCadence};
use fakeland_types::Dur;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use super::common::{limit, ms};
use crate::params::{
    ClickParams, DragParams, KeyParams, PointParams, ScrollParams, TypeTextParams,
};
use crate::reply::{ToolResult, finish};
use crate::server::FakelandMcp;
use crate::state::{CALL_SLACK, bounded};

/// Parses a button name.
pub fn button(name: Option<&str>) -> Result<Button> {
    match name {
        None => Ok(Button::Left),
        Some(n) => serde_json::from_value(serde_json::json!(n.trim().to_ascii_lowercase()))
            .with_context(|| format!("unknown button {n:?}")),
    }
}

fn sent(n: &[SentInput]) -> String {
    if n.is_empty() {
        " (no client received it: no window under the pointer or with focus)".to_owned()
    } else {
        String::new()
    }
}

impl FakelandMcp {
    async fn click_at(
        &self,
        p: PointParams,
        button: Button,
        count: u32,
    ) -> Result<rmcp::model::CallToolResult> {
        let c = self.ctl().await?;
        let (pos, desc) = self
            .state
            .position(&c, p.x, p.y, p.logical, p.window.as_deref())
            .await?;
        let events = bounded(
            CALL_SLACK,
            c.click(CtlClick {
                pos: Some(pos),
                button,
                count: count.max(1),
            }),
        )
        .await??;
        let what = match count {
            0 | 1 => "Clicked",
            2 => "Double-clicked",
            _ => "Clicked repeatedly",
        };
        self.after(
            &c,
            p.screenshot,
            format!("{what} {button:?} at {desc}{}", sent(&events)),
        )
        .await
    }

    async fn move_to(&self, p: PointParams, verb: &str) -> Result<rmcp::model::CallToolResult> {
        let c = self.ctl().await?;
        let (pos, desc) = self
            .state
            .position(&c, p.x, p.y, p.logical, p.window.as_deref())
            .await?;
        let events = bounded(CALL_SLACK, c.session().pointer_move(pos)).await??;
        self.after(
            &c,
            p.screenshot,
            format!("{verb} to {desc}{}", sent(&events)),
        )
        .await
    }
}

#[tool_router(router = input_router, vis = "pub(crate)")]
impl FakelandMcp {
    /// Clicks.
    #[tool(
        description = "Click at a point: pixel coordinates of the last screenshot by default. button: left (default), right, middle. count 2 double-clicks."
    )]
    async fn click(&self, Parameters(p): Parameters<ClickParams>) -> ToolResult {
        finish(
            async {
                let b = button(p.button.as_deref())?;
                self.click_at(
                    PointParams {
                        x: p.x,
                        y: p.y,
                        logical: p.logical,
                        window: p.window.clone(),
                        screenshot: p.screenshot,
                    },
                    b,
                    p.count,
                )
                .await
            }
            .await,
        )
    }

    /// Double-clicks.
    #[tool(
        description = "Double-click the left button at a point: pixel coordinates of the last screenshot by default."
    )]
    async fn double_click(&self, Parameters(p): Parameters<PointParams>) -> ToolResult {
        finish(self.click_at(p, Button::Left, 2).await)
    }

    /// Right-clicks.
    #[tool(description = "Click the right button at a point, for example to open a context menu.")]
    async fn right_click(&self, Parameters(p): Parameters<PointParams>) -> ToolResult {
        finish(self.click_at(p, Button::Right, 1).await)
    }

    /// Moves the pointer.
    #[tool(
        description = "Move the pointer to a point without clicking: pixel coordinates of the last screenshot by default."
    )]
    async fn move_pointer(&self, Parameters(p): Parameters<PointParams>) -> ToolResult {
        finish(self.move_to(p, "Moved").await)
    }

    /// Hovers.
    #[tool(
        description = "Hover over a point, for example to show a tooltip or a hover state. Same as move_pointer."
    )]
    async fn hover(&self, Parameters(p): Parameters<PointParams>) -> ToolResult {
        finish(self.move_to(p, "Hovered").await)
    }

    /// Drags.
    #[tool(
        description = "Drag with a button held along path [[x, y], ...] (start, optional middle points, end) over duration_ms. Pixel coordinates of the last screenshot by default."
    )]
    async fn drag(&self, Parameters(p): Parameters<DragParams>) -> ToolResult {
        finish(
            async {
                if p.path.len() < 2 {
                    bail!("the path needs at least a start and an end point");
                }
                let c = self.ctl().await?;
                let mut points = Vec::with_capacity(p.path.len());
                let mut first = String::new();
                for [x, y] in &p.path {
                    let (pos, desc) = self
                        .state
                        .position(&c, *x, *y, p.logical, p.window.as_deref())
                        .await?;
                    if first.is_empty() {
                        first = desc;
                    }
                    points.push(pos);
                }
                let duration = Dur(ms(p.duration_ms.max(1)));
                let last = points.last().cloned().context("empty path")?;
                let mut drag = Drag::line(points[0].clone(), last, duration);
                if points.len() > 2 {
                    drag.path = Path::Polyline { points };
                }
                drag.button = button(p.button.as_deref())?;
                if let Some(rate) = c.session().config().input.pointer.report_rate {
                    drag.rate_hz = rate.0;
                }
                let outcome =
                    bounded(limit(p.duration_ms), c.session().act(Action::Drag(drag))).await??;
                let n = match outcome {
                    fakeland::ActionOutcome::Input { sent } => sent.len(),
                    _ => 0,
                };
                self.after(
                    &c,
                    p.screenshot,
                    format!(
                        "Dragged from {first} through {} points in {} ms, {n} events",
                        p.path.len(),
                        p.duration_ms
                    ),
                )
                .await
            }
            .await,
        )
    }

    /// Scrolls.
    #[tool(
        description = "Scroll by dx/dy logical pixels (positive dy scrolls down) at the pointer, or at (x, y) first. smooth_ms spreads it over time like a real wheel; detents uses a classic wheel, finger a touchpad."
    )]
    async fn scroll(&self, Parameters(p): Parameters<ScrollParams>) -> ToolResult {
        finish(
            async {
                if p.dx == 0.0 && p.dy == 0.0 {
                    bail!("give dx or dy");
                }
                let c = self.ctl().await?;
                let (at, desc) = match (p.x, p.y) {
                    (Some(x), Some(y)) => {
                        let (pos, desc) = self
                            .state
                            .position(&c, x, y, p.logical, p.window.as_deref())
                            .await?;
                        (Some(pos), format!(" at {desc}"))
                    }
                    (None, None) => (None, String::new()),
                    _ => bail!("give both x and y, or neither"),
                };
                let source = if p.finger {
                    ScrollSource::finger(true)
                } else {
                    ScrollSource::wheel(!p.detents)
                };
                let smooth = p.smooth_ms.filter(|m| *m > 0);
                let events = bounded(
                    limit(smooth.unwrap_or(0)),
                    c.scroll(CtlScroll {
                        at,
                        dx: p.dx,
                        dy: p.dy,
                        source,
                        smooth: smooth.map(|m| Dur(ms(m))),
                    }),
                )
                .await??;
                self.after(
                    &c,
                    p.screenshot,
                    format!(
                        "Scrolled dx={} dy={}{desc}, {} events{}",
                        p.dx,
                        p.dy,
                        events.len(),
                        sent(&events)
                    ),
                )
                .await
            }
            .await,
        )
    }

    /// Types text.
    #[tool(
        description = "Type text into the focused window as key presses. Characters without a key on the layout still arrive. Click a text field first to focus it."
    )]
    async fn type_text(&self, Parameters(p): Parameters<TypeTextParams>) -> ToolResult {
        finish(
            async {
                if !(p.wpm.is_finite() && p.wpm > 0.0) {
                    bail!("wpm must be positive");
                }
                let c = self.ctl().await?;
                let chars = p.text.chars().count() as f64;
                let expected_ms = (chars * 12_000.0 / p.wpm) as u64;
                let events = bounded(
                    limit(expected_ms),
                    c.type_text(CtlType {
                        text: p.text.clone(),
                        cadence: TypingCadence::wpm(p.wpm),
                        via: TextVia::Keys,
                    }),
                )
                .await??;
                self.after(
                    &c,
                    p.screenshot,
                    format!("Typed {} characters{}", chars, sent(&events)),
                )
                .await
            }
            .await,
        )
    }

    /// Presses a key or combination.
    #[tool(
        description = "Press a key or key combination in the focused window: Return, Escape, Tab, BackSpace, Delete, Up, Down, Page_Down, F5, ctrl+c, ctrl+shift+t, alt+F4."
    )]
    async fn key(&self, Parameters(p): Parameters<KeyParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let events = bounded(
                    CALL_SLACK,
                    c.combo(ComboParams {
                        combo: p.combo.clone(),
                    }),
                )
                .await??;
                self.after(
                    &c,
                    p.screenshot,
                    format!("Pressed {}{}", p.combo, sent(&events)),
                )
                .await
            }
            .await,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_buttons() {
        assert_eq!(button(None).unwrap(), Button::Left);
        assert_eq!(button(Some("Right")).unwrap(), Button::Right);
        assert!(button(Some("pinky")).is_err());
    }
}
