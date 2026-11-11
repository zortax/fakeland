//! The viewer on the compositor thread: it starts and stops the viewer thread, keeps the output
//! list of the viewer current, turns viewer messages into session input and picture requests,
//! and refreshes the window titles with the frame rate of the focused window.

use std::collections::HashMap;
use std::thread::JoinHandle;

use fakeland_trace::{Event, InputKind, Record};
use fakeland_types::{Nanos, OutputId, WindowId};
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::calloop::channel::{self, Channel};

use super::compose::{OutputLink, Path};
use super::fps::FpsMeter;
use super::mailbox::{MailSender, mailbox};
use super::title::{TitleInfo, title};
use super::{FromViewer, OutputView, ToViewer};
use crate::display::TimerId;
use crate::server::State;
use crate::waits::{Observe, ObserverId};

/// The title refresh interval.
const TITLE_EVERY: Nanos = Nanos::from_millis(500);

/// The viewer as the compositor thread sees it.
#[derive(Debug)]
pub struct ViewerLink {
    pub(super) to: MailSender<ToViewer>,
    thread: Option<JoinHandle<()>>,
    source: RegistrationToken,
    observer: ObserverId,
    timer: Option<TimerId>,
    pub(super) formats: Option<Vec<(u32, u64)>>,
    pub(super) outputs: HashMap<OutputId, OutputLink>,
    fps: FpsMeter,
    fps_window: Option<WindowId>,
}

impl State {
    /// Starts the viewer when the config asks for it.
    pub fn start_viewer(&mut self) -> Result<(), String> {
        if !self.config.viewer {
            return Ok(());
        }
        let (to, to_rx) = mailbox().map_err(|e| e.to_string())?;
        let (from_tx, from_rx): (_, Channel<FromViewer>) = channel::channel();
        let source = self
            .loop_handle
            .insert_source(from_rx, |event, _, state| {
                if let channel::Event::Msg(msg) = event {
                    state.on_viewer(msg);
                }
            })
            .map_err(|e| e.error.to_string())?;
        let thread = super::client::spawn(self.config.viewer_display.clone(), to_rx, from_tx)
            .map_err(|e| e.to_string())?;
        let observer = self.observe(|state, record| state.viewer_record(record));
        self.viewer = Some(ViewerLink {
            to,
            thread: Some(thread),
            source,
            observer,
            timer: None,
            formats: None,
            outputs: HashMap::new(),
            fps: FpsMeter::default(),
            fps_window: None,
        });
        self.viewer_outputs();
        self.viewer_titles();
        Ok(())
    }

    /// Stops the viewer and waits for its thread.
    pub fn stop_viewer(&mut self) {
        let Some(mut link) = self.viewer.take() else {
            return;
        };
        link.to.send(ToViewer::Stop);
        self.unobserve(link.observer);
        self.loop_handle.remove(link.source);
        if let Some(t) = link.timer {
            self.cancel_timer(t);
        }
        if let Some(thread) = link.thread.take() {
            let _ = thread.join();
        }
    }

    /// Returns true while the viewer runs.
    pub fn viewer_running(&self) -> bool {
        self.viewer.is_some()
    }

    /// Sends the output list to the viewer.
    fn viewer_outputs(&mut self) {
        let views: Vec<OutputView> = self
            .outputs
            .iter()
            .filter(|o| o.enabled())
            .map(|o| {
                let r = o.logical_rect();
                OutputView {
                    id: o.id,
                    name: o.config.name.clone(),
                    logical: (r.w, r.h),
                }
            })
            .collect();
        let Some(link) = &mut self.viewer else {
            return;
        };
        link.outputs
            .retain(|id, _| views.iter().any(|v| v.id == *id));
        for v in &views {
            link.outputs.entry(v.id).or_insert(OutputLink {
                path: Path::Unknown,
                wants: false,
                dirty: true,
                scheduled: false,
                shown: (v.logical.0.max(1) as u32, v.logical.1.max(1) as u32),
                title: String::new(),
            });
        }
        link.to.send(ToViewer::Outputs(views));
    }

    fn on_viewer(&mut self, msg: FromViewer) {
        match msg {
            FromViewer::Started { formats } => {
                tracing::info!(
                    "viewer started, host imports {} dmabuf formats",
                    formats.len()
                );
                if let Some(link) = &mut self.viewer {
                    link.formats = Some(formats);
                }
            }
            FromViewer::Ready {
                output,
                width,
                height,
            } => {
                if let Some(o) = self
                    .viewer
                    .as_mut()
                    .and_then(|l| l.outputs.get_mut(&output))
                {
                    o.wants = true;
                    o.shown = (width.max(1), height.max(1));
                }
                self.viewer_dirty(output);
            }
            FromViewer::Released { output, id } => {
                if let Some(OutputLink {
                    path: Path::Gpu(c), ..
                }) = self
                    .viewer
                    .as_mut()
                    .and_then(|l| l.outputs.get_mut(&output))
                {
                    c.release(id);
                }
            }
            FromViewer::DmabufFailed { output } => {
                if let Some(o) = self
                    .viewer
                    .as_mut()
                    .and_then(|l| l.outputs.get_mut(&output))
                {
                    o.path = Path::Cpu;
                    o.dirty = true;
                }
            }
            FromViewer::Input { output, input } => self.inject_viewer_input(output, input),
            FromViewer::Ended(error) => {
                match error {
                    Some(e) => tracing::warn!("the viewer ended: {e}"),
                    None => tracing::info!("the viewer window closed"),
                }
                self.stop_viewer();
            }
        }
    }

    fn viewer_record(&mut self, record: &Record) -> Observe {
        let Some(link) = &self.viewer else {
            return Observe::Remove;
        };
        let focus = self.windows.focus;
        let fps_window = link.fps_window;
        match &record.event {
            Event::Presented {
                surface,
                output,
                time,
                ..
            } => {
                if focus.is_some() && self.window_of_surface_id(*surface) == focus {
                    if let Some(link) = &mut self.viewer {
                        if link.fps_window != focus {
                            link.fps.clear();
                            link.fps_window = focus;
                        }
                        link.fps.record(*time);
                    }
                } else if fps_window != focus
                    && let Some(link) = &mut self.viewer
                {
                    link.fps.clear();
                    link.fps_window = focus;
                }
                self.viewer_dirty(*output);
            }
            Event::OutputConfigured(_)
            | Event::OutputChanged(_)
            | Event::OutputRemoved { .. }
            | Event::OutputPower { .. } => {
                self.viewer_outputs();
                self.viewer_dirty_all();
            }
            Event::WindowMapped { .. } | Event::WindowUnmapped { .. } => self.viewer_dirty_all(),
            Event::InputSent {
                kind: InputKind::PointerMotion | InputKind::PointerEnter,
                ..
            } => self.viewer_dirty_all(),
            _ => {}
        }
        Observe::Keep
    }

    /// Updates every window title and arms the next update.
    fn viewer_titles(&mut self) {
        let now = self.now();
        let focus = self
            .windows
            .focus
            .and_then(|id| self.windows.get(id))
            .map(|r| self.window_info(r));
        let Some(link) = &mut self.viewer else {
            return;
        };
        let fps = link.fps.fps(now);
        for out in self.outputs.iter() {
            let Some(o) = link.outputs.get_mut(&out.id) else {
                continue;
            };
            let app = focus.as_ref().filter(|w| w.output == out.id).map(|w| {
                let name = if w.app_id.is_empty() {
                    w.title.as_str()
                } else {
                    w.app_id.as_str()
                };
                (name, fps)
            });
            let text = title(&TitleInfo {
                output: &out.config.name,
                width: out.config.mode.width,
                height: out.config.mode.height,
                refresh: out.config.mode.refresh.0,
                scale: out.config.scale,
                focused: app,
            });
            if text != o.title {
                o.title = text.clone();
                link.to.send(ToViewer::Title {
                    output: out.id,
                    title: text,
                });
            }
        }
        let timer = self.schedule_in(TITLE_EVERY, |state| state.viewer_titles());
        if let Some(link) = &mut self.viewer {
            link.timer = Some(timer);
        }
    }
}
