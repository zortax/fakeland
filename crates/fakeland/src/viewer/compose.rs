//! Pictures of the viewer outputs on the compositor thread. An output composites when its
//! window asked for a picture and the output changed since the last one: through the GPU
//! composer when the host imports dmabufs, or as CPU pixels at the shown size.

use std::sync::Arc;

use fakeland_scenario::CaptureTarget;
use fakeland_trace::Event;
use fakeland_types::OutputId;

use super::composer::Composer;
use super::layout::shrink_factor;
use super::{ToViewer, ViewerFrame};
use crate::server::State;

/// How an output reaches its window.
#[derive(Debug)]
pub(super) enum Path {
    /// The composite waits for the host formats.
    Unknown,
    /// GPU buffers.
    Gpu(Box<Composer>),
    /// CPU pixels.
    Cpu,
}

/// The viewer state of one output.
#[derive(Debug)]
pub(super) struct OutputLink {
    pub(super) path: Path,
    pub(super) wants: bool,
    pub(super) dirty: bool,
    pub(super) scheduled: bool,
    pub(super) shown: (u32, u32),
    pub(super) title: String,
}

impl State {
    pub(super) fn viewer_dirty_all(&mut self) {
        let ids: Vec<OutputId> = self
            .viewer
            .as_ref()
            .map(|l| l.outputs.keys().copied().collect())
            .unwrap_or_default();
        for id in ids {
            self.viewer_dirty(id);
        }
    }

    pub(super) fn viewer_dirty(&mut self, output: OutputId) {
        let Some(o) = self
            .viewer
            .as_mut()
            .and_then(|l| l.outputs.get_mut(&output))
        else {
            return;
        };
        o.dirty = true;
        if !o.wants || o.scheduled {
            return;
        }
        o.scheduled = true;
        let now = self.now();
        self.schedule_at(now, move |state| state.viewer_compose_output(output));
    }

    fn viewer_compose_output(&mut self, output: OutputId) {
        let Some(link) = &mut self.viewer else {
            return;
        };
        let formats = link.formats.clone();
        let Some(o) = link.outputs.get_mut(&output) else {
            return;
        };
        o.scheduled = false;
        if !(o.wants && o.dirty) {
            return;
        }
        if matches!(o.path, Path::Unknown) {
            let Some(formats) = formats else {
                return;
            };
            o.path = Path::Cpu;
            if !formats.is_empty() {
                match self.viewer_composer(output, &formats) {
                    Ok(c) => self.viewer_set_path(output, Path::Gpu(Box::new(c))),
                    Err(e) => tracing::info!("viewer uses CPU frames: {e}"),
                }
            }
        }
        self.viewer_refit(output);
        let start = self.now();
        let Some(path) = self
            .viewer
            .as_mut()
            .and_then(|l| l.outputs.get_mut(&output))
            .map(|o| std::mem::replace(&mut o.path, Path::Cpu))
        else {
            return;
        };
        let (path, frame) = match path {
            Path::Gpu(mut c) => {
                let frame = self.viewer_compose(&mut c);
                (Path::Gpu(c), frame)
            }
            other => {
                let shown = self
                    .viewer
                    .as_ref()
                    .and_then(|l| l.outputs.get(&output))
                    .map(|o| o.shown);
                let frame = self.viewer_pixels(output, shown.unwrap_or((1, 1)));
                (other, frame)
            }
        };
        let end = self.now();
        self.emit_at(
            end,
            Event::CompositorOverhead {
                what: "viewer composite".into(),
                duration: end.since(start),
            },
        );
        let Some(link) = &mut self.viewer else {
            return;
        };
        let Some(o) = link.outputs.get_mut(&output) else {
            return;
        };
        o.path = path;
        match frame {
            Ok(Some(frame)) => {
                o.dirty = false;
                o.wants = false;
                link.to.send(ToViewer::Frame { output, frame });
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("viewer composite failed, using CPU frames: {e}");
                o.path = Path::Cpu;
            }
        }
    }

    fn viewer_set_path(&mut self, output: OutputId, path: Path) {
        if let Some(o) = self
            .viewer
            .as_mut()
            .and_then(|l| l.outputs.get_mut(&output))
        {
            o.path = path;
        }
    }

    /// Rebuilds the composer of an output whose size or scale changed.
    fn viewer_refit(&mut self, output: OutputId) {
        let Some(out) = self.outputs.get(output) else {
            return;
        };
        let (region, scale) = (out.logical_rect(), out.config.scale);
        let Some(link) = &mut self.viewer else {
            return;
        };
        let formats = link.formats.clone().unwrap_or_default();
        let stale = match link.outputs.get(&output).map(|o| &o.path) {
            Some(Path::Gpu(c)) if !c.fits(region, scale) => c.ids().to_vec(),
            _ => return,
        };
        link.to.send(ToViewer::Retire { output, ids: stale });
        let path = match self.viewer_composer(output, &formats) {
            Ok(c) => Path::Gpu(Box::new(c)),
            Err(_) => Path::Cpu,
        };
        self.viewer_set_path(output, path);
    }

    fn viewer_pixels(
        &mut self,
        output: OutputId,
        shown: (u32, u32),
    ) -> Result<Option<ViewerFrame>, String> {
        let target = CaptureTarget::Output {
            output: Some(output),
        };
        let (w, h) = self.capture_size(&target, 1.0).map_err(|e| e.to_string())?;
        let factor = shrink_factor((w as i32, h as i32), shown);
        let shot = self
            .capture_scaled(&target, true, factor)
            .map_err(|e| e.to_string())?;
        Ok(Some(ViewerFrame::Pixels(Arc::new(shot.image))))
    }
}
