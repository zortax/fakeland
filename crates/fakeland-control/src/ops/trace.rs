//! Trace operations: span captures, snapshots of the recent past and frame statistics.

use fakeland::CaptureId;
use fakeland_trace::Trace;

use super::{Controller, OpError};
use crate::export::{export_trace, trace_info};
use crate::methods::{
    FrameStats, FrameStatsParams, NoParams, TraceCapture, TraceEndParams, TraceFileInfo,
    TraceOutput, TraceSnapshotParams,
};
use crate::text::summary_text;

fn write(trace: &Trace, output: Option<&TraceOutput>) -> Result<TraceFileInfo, OpError> {
    match output {
        None => Ok(trace_info(trace)),
        Some(out) if !out.path.is_absolute() => Err(OpError::InvalidParams(format!(
            "the trace path must be absolute: {}",
            out.path.display()
        ))),
        Some(out) => export_trace(trace, out).map_err(|e| OpError::Failed(e.to_string())),
    }
}

impl Controller {
    fn open_captures(&self) -> std::sync::MutexGuard<'_, Vec<u64>> {
        self.inner
            .captures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Starts a trace capture.
    pub async fn trace_begin(&self, _: NoParams) -> Result<TraceCapture, OpError> {
        let id = self.session().begin_capture().await?;
        self.open_captures().push(id.0);
        Ok(TraceCapture { capture: id.0 })
    }

    /// Ends a trace capture and writes it.
    pub async fn trace_end(&self, p: TraceEndParams) -> Result<TraceFileInfo, OpError> {
        if let Some(out) = &p.output
            && !out.path.is_absolute()
        {
            return Err(OpError::InvalidParams(format!(
                "the trace path must be absolute: {}",
                out.path.display()
            )));
        }
        let capture = {
            let mut open = self.open_captures();
            match p.capture {
                Some(id) => {
                    open.retain(|c| *c != id);
                    id
                }
                None => open
                    .pop()
                    .ok_or_else(|| OpError::NotFound("no trace capture is open".into()))?,
            }
        };
        let trace = self
            .session()
            .end_capture(CaptureId(capture))
            .await
            .map_err(|e| match e {
                fakeland::SessionError::Trace(_) => {
                    OpError::NotFound(format!("no trace capture {capture}"))
                }
                e => e.into(),
            })?;
        write(&trace, p.output.as_ref())
    }

    /// Returns the buffered trace, or its last span.
    pub async fn recent_trace(&self, last: Option<fakeland_types::Dur>) -> Result<Trace, OpError> {
        let trace = self.session().trace_snapshot().await?;
        Ok(match last {
            Some(span) => {
                let now = self.session().clock().now();
                trace.slice(now - span.0..)
            }
            None => trace,
        })
    }

    /// Writes the buffered trace or its last span.
    pub async fn trace_snapshot(&self, p: TraceSnapshotParams) -> Result<TraceFileInfo, OpError> {
        let trace = self.recent_trace(p.last).await?;
        write(&trace, p.output.as_ref())
    }

    /// Summarizes the frames of one window over a recent span.
    pub async fn frame_stats(&self, p: FrameStatsParams) -> Result<FrameStats, OpError> {
        let window = self.resolve_window(p.window).await?;
        let trace = self.recent_trace(Some(p.last)).await?.for_window(window.id);
        let info = trace_info(&trace);
        Ok(FrameStats {
            text: summary_text(&info.summary),
            span_ms: p.last.0.as_millis_f64(),
            summary: info.summary,
            window,
        })
    }
}
