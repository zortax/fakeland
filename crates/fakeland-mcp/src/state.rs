//! The session of the MCP server: started on demand, replaced on request, stopped at exit. It
//! also keeps the pixel frame of the last screenshot for coordinate mapping.

use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use fakeland::{Fakeland, SessionConfig};
use fakeland_control::{Controller, SessionSpec};
use fakeland_scenario::Position;
use fakeland_types::OutputId;

use crate::coords::{OutputArea, ShotFrame, global_to_output};

/// The longest time any single session call may take beyond its own timeout.
pub const CALL_SLACK: Duration = Duration::from_secs(5);

/// The state shared by every tool call.
#[derive(Debug, Default)]
pub struct McpState {
    live: tokio::sync::Mutex<Option<Controller>>,
    shot: Mutex<Option<ShotFrame>>,
}

/// Runs a session call with a time limit.
pub async fn bounded<T>(limit: Duration, fut: impl Future<Output = T>) -> Result<T> {
    tokio::time::timeout(limit, fut)
        .await
        .map_err(|_| anyhow!("the call took longer than {:.1} s", limit.as_secs_f64()))
}

impl McpState {
    /// Returns the running session, or starts one with default settings.
    pub async fn controller(&self) -> Result<Controller> {
        let mut live = self.live.lock().await;
        if let Some(c) = live.as_ref() {
            return Ok(c.clone());
        }
        let c = start(SessionConfig::default()).await?;
        *live = Some(c.clone());
        Ok(c)
    }

    /// Starts a session from a spec. An existing session stops first when `restart` is set;
    /// otherwise an existing session is an error.
    pub async fn start(&self, spec: &SessionSpec, restart: bool) -> Result<Controller> {
        let config = spec.apply(SessionConfig::default())?;
        let mut live = self.live.lock().await;
        if let Some(old) = live.take() {
            if !restart {
                *live = Some(old);
                bail!("a session already runs; pass restart=true to replace it");
            }
            stop(old).await;
            self.forget_shot();
        }
        let c = start(config).await?;
        *live = Some(c.clone());
        Ok(c)
    }

    /// Stops the session. Returns false when none ran.
    pub async fn stop(&self) -> bool {
        let old = self.live.lock().await.take();
        self.forget_shot();
        match old {
            Some(c) => {
                stop(c).await;
                true
            }
            None => false,
        }
    }

    /// Records the frame of a new screenshot.
    pub fn set_shot(&self, frame: ShotFrame) {
        *self.shot.lock().unwrap_or_else(|e| e.into_inner()) = Some(frame);
    }

    /// Returns the frame of the last screenshot.
    pub fn shot(&self) -> Option<ShotFrame> {
        *self.shot.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn forget_shot(&self) {
        *self.shot.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Resolves tool coordinates to a position and a description. Pixel coordinates refer to
    /// the last screenshot; logical coordinates refer to a window or to an output.
    pub async fn position(
        &self,
        c: &Controller,
        x: f64,
        y: f64,
        logical: bool,
        window: Option<&str>,
    ) -> Result<(Position, String)> {
        if let Some(w) = window {
            let matcher = fakeland_control::spec::parse_window_ref(w)?;
            return Ok((
                Position::Window {
                    window: matcher,
                    x,
                    y,
                },
                format!("window-relative logical ({x:.1}, {y:.1})"),
            ));
        }
        let outputs = bounded(CALL_SLACK, c.session().outputs()).await??;
        let areas: Vec<OutputArea> = outputs.iter().map(OutputArea::of).collect();
        let shot = self.shot();
        let (gx, gy, prefer, note) = match (logical, shot) {
            (false, Some(f)) => {
                if !f.contains(x, y) {
                    bail!(
                        "pixel ({x}, {y}) lies outside the last screenshot of {}x{} px",
                        f.size.0,
                        f.size.1
                    );
                }
                let (gx, gy) = f.to_global(x, y);
                (
                    gx,
                    gy,
                    f.output,
                    format!("screenshot pixel ({x:.1}, {y:.1}) = "),
                )
            }
            (false, None) => {
                let first = areas.first().context("the session has no output")?;
                (
                    first.x + x,
                    first.y + y,
                    Some(first.id),
                    "no screenshot yet, so the coordinates are logical: ".to_owned(),
                )
            }
            (true, _) => {
                let base = shot
                    .and_then(|f| f.output)
                    .and_then(|id| areas.iter().find(|a| a.id == id))
                    .or_else(|| areas.first())
                    .context("the session has no output")?;
                (base.x + x, base.y + y, Some(base.id), String::new())
            }
        };
        let (output, ox, oy) =
            global_to_output(&areas, gx, gy, prefer).context("the session has no output")?;
        Ok((
            Position::Output {
                output: Some(output),
                x: ox,
                y: oy,
            },
            format!(
                "{note}logical ({ox:.1}, {oy:.1}) on {}",
                name(&outputs, output)
            ),
        ))
    }
}

fn name(outputs: &[fakeland::OutputState], id: OutputId) -> String {
    outputs
        .iter()
        .find(|o| o.id == id)
        .map_or_else(|| id.to_string(), |o| o.name.clone())
}

async fn start(config: SessionConfig) -> Result<Controller> {
    let session = tokio::task::spawn_blocking(move || Fakeland::start(config))
        .await
        .context("the start thread failed")?
        .context("cannot start the session")?;
    Ok(Controller::new(session))
}

async fn stop(c: Controller) {
    let session = c.session().clone();
    drop(c);
    let _ = tokio::task::spawn_blocking(move || session.blocking_shutdown()).await;
}
