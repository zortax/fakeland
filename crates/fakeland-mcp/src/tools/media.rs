//! Resource and recording tools: the latest CPU, memory and GPU samples, and video recordings
//! of an output or a window that follow the display refresh.

use anyhow::Result;
use fakeland::{RecordSpec, RecordTarget, RecordingId};
use fakeland_control::methods::{NoParams, RecordStopParams as CtlRecordStop};
use fakeland_control::spec::parse_window_ref;
use fakeland_types::{Hz, OutputId};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use crate::params::{NoArgs, RecordStartParams, RecordStopParams};
use crate::reply::{ToolResult, done_json, finish};
use crate::server::FakelandMcp;
use crate::state::{CALL_SLACK, bounded};

/// The longest wait for an encoder to finish its file.
const STOP_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

fn record_spec(p: &RecordStartParams) -> Result<RecordSpec> {
    let target = match &p.window {
        Some(w) => RecordTarget::Window {
            window: parse_window_ref(w)?,
        },
        None => RecordTarget::Output {
            output: p.output.map(OutputId),
        },
    };
    Ok(RecordSpec {
        target,
        path: std::path::absolute(&p.path)?,
        format: None,
        fps: p.fps.map(Hz),
        max_dim: p.max_dim,
        include_cursor: p.cursor,
    })
}

#[tool_router(router = media_router, vis = "pub(crate)")]
impl FakelandMcp {
    /// Returns the latest resource samples.
    #[tool(
        description = "Show the latest CPU (percent of one core), resident memory and thread count of every launched app with its child processes, and GPU utilization and memory. Needs a session started with sample, such as session_start with sample=\"100ms\"; the samples also go into traces as counters."
    )]
    async fn resources(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let snap = bounded(CALL_SLACK, c.resources(NoParams {})).await??;
                done_json(snap.to_text(), &snap)
            }
            .await,
        )
    }

    /// Starts a recording.
    #[tool(
        description = "Start a video recording of the output or of one window. Frames follow the display refresh of the session: one frame per refresh, and a frame repeats while nothing new shows, so the video plays at real speed. Returns the recording id; stop it with record_stop."
    )]
    async fn record_start(&self, Parameters(p): Parameters<RecordStartParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let spec = record_spec(&p)?;
                let info = bounded(CALL_SLACK, c.record_start(spec)).await??;
                done_json(
                    format!(
                        "Recording {} started: {} at {}x{}, {:.3} fps.",
                        info.id.0,
                        info.path.display(),
                        info.width,
                        info.height,
                        info.rate.hz().0
                    ),
                    &info,
                )
            }
            .await,
        )
    }

    /// Stops a recording.
    #[tool(
        description = "Stop a recording and wait until its file is complete. Reports the frame count, how many frames show new content, repeats and frames dropped because the encoder fell behind."
    )]
    async fn record_stop(&self, Parameters(p): Parameters<RecordStopParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let report = bounded(
                    STOP_LIMIT,
                    c.record_stop(CtlRecordStop {
                        id: p.id.map(RecordingId),
                    }),
                )
                .await??;
                done_json(report.to_text(), &report)
            }
            .await,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_take_absolute_paths_and_targets() {
        let p = RecordStartParams {
            path: "out.mp4".into(),
            window: Some("app:foot".into()),
            output: None,
            fps: Some(30.0),
            max_dim: Some(640),
            cursor: false,
        };
        let s = record_spec(&p).unwrap();
        assert!(s.path.is_absolute());
        assert!(matches!(s.target, RecordTarget::Window { .. }));
        assert_eq!(s.fps, Some(Hz(30.0)));
    }
}
