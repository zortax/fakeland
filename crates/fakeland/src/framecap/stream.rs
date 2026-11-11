//! Frame capture on the compositor thread. A commit capture renders the window region on the loop
//! turn after smithay applies a commit of its tree with a new buffer; a presented capture renders
//! it at each refresh that shows a new buffer of the tree. Both render the region at the output
//! scale and box-filter it.

use fakeland_scenario::{CaptureTarget, Roi, WindowRef};
use fakeland_types::{CommitId, SurfaceId, Timestamp};

use super::{ActiveCapture, CaptureSpec, CapturedFrame, FrameCapture, FrameCaptureId, FrameSink};
use crate::capture::{CaptureError, CaptureOptions};
use crate::frames::CommitPhase;
use crate::server::State;

impl State {
    /// Starts a frame capture. A sink takes the frames instead of the capture keeping them.
    pub fn begin_frame_capture(
        &mut self,
        spec: CaptureSpec,
        sink: Option<FrameSink>,
    ) -> Result<FrameCaptureId, CaptureError> {
        let window = self
            .resolve_window(spec.window())?
            .ok_or(CaptureError::NoWindow)?;
        let id = self.frame_captures.next_id();
        let hook = match &spec {
            CaptureSpec::EveryCommit { .. } => Some(self.add_commit_hook(move |state, event| {
                if event.phase != CommitPhase::Applied
                    || state.window_of_surface(event.surface) != Some(window)
                {
                    return;
                }
                let Some(commit) = event.newest_buffer else {
                    return;
                };
                let surface = event.id;
                let at = state.now();
                state.schedule_at(at, move |state| {
                    state.take_frame(id, commit, surface, None);
                });
            })),
            CaptureSpec::EveryPresented { .. } => None,
        };
        self.frame_captures.active.insert(
            id,
            ActiveCapture {
                spec,
                window,
                frames: Vec::new(),
                taken: 0,
                dropped: 0,
                cost: fakeland_types::Nanos::ZERO,
                hook,
                sink,
            },
        );
        Ok(id)
    }

    /// Ends a frame capture and returns its frames.
    pub fn end_frame_capture(&mut self, id: FrameCaptureId) -> Option<FrameCapture> {
        let active = self.frame_captures.active.remove(&id)?;
        if let Some(h) = active.hook {
            self.remove_commit_hook(h);
        }
        Some(FrameCapture {
            spec: active.spec,
            frames: active.frames,
            dropped: active.dropped,
            cost: active.cost,
        })
    }

    /// Takes a frame for every presented capture of the window of `surface` once the current
    /// wake ends. `content` is the commit whose new buffer the refresh `seq` shows.
    pub fn capture_presented(
        &mut self,
        surface: SurfaceId,
        content: CommitId,
        time: Timestamp,
        seq: u64,
    ) {
        if self.frame_captures.active.is_empty() {
            return;
        }
        let Some(window) = self.window_of_surface_id(surface) else {
            return;
        };
        let mut ids: Vec<FrameCaptureId> = self
            .frame_captures
            .active
            .iter()
            .filter(|(_, c)| {
                c.window == window && matches!(c.spec, CaptureSpec::EveryPresented { .. })
            })
            .map(|(id, _)| *id)
            .collect();
        ids.sort();
        let at = self.now();
        self.schedule_at(at, move |state| {
            for id in ids {
                state.take_frame(id, content, surface, Some((time, seq)));
            }
        });
    }

    /// Renders one frame of a capture.
    fn take_frame(
        &mut self,
        id: FrameCaptureId,
        commit: CommitId,
        surface: SurfaceId,
        presented: Option<(Timestamp, u64)>,
    ) {
        let Some(active) = self.frame_captures.active.get_mut(&id) else {
            return;
        };
        if active.taken >= active.spec.max_frames() {
            active.dropped += 1;
            return;
        }
        let (window, roi, factor) = (active.window, active.spec.roi(), active.spec.downscale());
        let start = self.now();
        let shot = self.capture(
            &CaptureTarget::Roi {
                roi: Roi::Window {
                    window: WindowRef::id(window),
                    rect: roi,
                },
            },
            CaptureOptions::default(),
        );
        let cost = self.now().since(start);
        let Some(active) = self.frame_captures.active.get_mut(&id) else {
            return;
        };
        active.cost += cost;
        let shot = match shot {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!("frame capture failed: {e}");
                active.dropped += 1;
                return;
            }
        };
        let image = if factor > 1 {
            shot.image.downscale(factor)
        } else {
            shot.image
        };
        let frame = CapturedFrame {
            commit,
            surface,
            window,
            time: shot.time,
            presented: presented.map(|(t, _)| t),
            seq: presented.map(|(_, s)| s),
            image,
            region: shot.region,
            scale: shot.scale * factor as f64,
        };
        active.taken += 1;
        match active.sink.take() {
            Some(mut sink) => {
                sink(self, frame);
                if let Some(active) = self.frame_captures.active.get_mut(&id) {
                    active.sink = Some(sink);
                }
            }
            None => active.frames.push(frame),
        }
    }
}
