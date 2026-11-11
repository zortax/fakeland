//! Trigger matchers. Each matcher reads trace records on the compositor thread. A region matcher
//! samples its region at the presentations that follow a latch and fires when the pixels changed
//! against the previous sample.

use fakeland_scenario::{CaptureTarget, Roi, Trigger, TriggerEvent};
use fakeland_trace::{Event, InputSource, Record};
use fakeland_types::{OutputId, SurfaceId};
use fakeland_vision::Image;

use crate::capture::CaptureOptions;
use crate::server::State;
use crate::shell::WindowMatcher;
use crate::waits::{changed_fraction, mark_name};

/// The compiled event of one trigger.
enum Matcher {
    Presented(Option<WindowMatcher>),
    Committed(Option<WindowMatcher>),
    RoiChanged {
        roi: Roi,
        threshold: f64,
        last: Option<Image>,
        dirty: bool,
    },
    InputSent,
    WindowMapped(WindowMatcher),
    AppMark(String),
    Vblank {
        every: u32,
        output: Option<OutputId>,
        seen: u32,
    },
}

/// One armed trigger.
pub struct Armed {
    /// The index in the scenario.
    pub index: usize,
    /// The trigger.
    pub trigger: Trigger,
    /// The firing count.
    pub fired: u32,
    matcher: Matcher,
}

/// The armed triggers of one run.
#[derive(Default)]
pub struct TriggerSet {
    /// The triggers in scenario order.
    pub list: Vec<Armed>,
}

impl std::fmt::Debug for TriggerSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TriggerSet")
            .field("len", &self.list.len())
            .finish()
    }
}

impl Armed {
    /// Compiles a trigger. A region trigger takes its first sample here.
    pub fn new(state: &mut State, index: usize, trigger: &Trigger) -> Result<Self, String> {
        let window = |w: &Option<fakeland_scenario::WindowRef>| {
            w.as_ref()
                .map(WindowMatcher::new)
                .transpose()
                .map_err(|e| e.to_string())
        };
        let matcher = match &trigger.on {
            TriggerEvent::Presented { window: w } => Matcher::Presented(window(w)?),
            TriggerEvent::Committed { window: w } => Matcher::Committed(window(w)?),
            TriggerEvent::RoiChanged { roi, threshold } => {
                let pinned = state.pin_roi(roi).unwrap_or_else(|_| roi.clone());
                let last = sample(state, &pinned);
                Matcher::RoiChanged {
                    roi: pinned,
                    threshold: *threshold,
                    last,
                    dirty: false,
                }
            }
            TriggerEvent::InputSent => Matcher::InputSent,
            TriggerEvent::WindowMapped { matcher } => {
                Matcher::WindowMapped(WindowMatcher::new(matcher).map_err(|e| e.to_string())?)
            }
            TriggerEvent::AppMark { name } => Matcher::AppMark(name.clone()),
            TriggerEvent::Vblank { every, output } => Matcher::Vblank {
                every: (*every).max(1),
                output: *output,
                seen: 0,
            },
        };
        Ok(Self {
            index,
            trigger: trigger.clone(),
            fired: 0,
            matcher,
        })
    }

    /// Returns true when the trigger reached its limit.
    pub fn exhausted(&self) -> bool {
        self.trigger.limit.is_some_and(|l| self.fired >= l)
    }

    /// Returns true when a record fires the trigger.
    pub fn matches(&mut self, state: &mut State, record: &Record) -> bool {
        if self.exhausted() {
            return false;
        }
        match &mut self.matcher {
            Matcher::Presented(w) => match &record.event {
                Event::Presented { surface, .. } => surface_matches(state, *surface, w.as_ref()),
                _ => false,
            },
            Matcher::Committed(w) => match &record.event {
                Event::Commit { surface, .. } => surface_matches(state, *surface, w.as_ref()),
                _ => false,
            },
            Matcher::RoiChanged {
                roi,
                threshold,
                last,
                dirty,
            } => match &record.event {
                Event::Latched { .. } | Event::WindowMapped { .. } => {
                    *dirty = true;
                    false
                }
                Event::Presented { .. } if *dirty => {
                    *dirty = false;
                    let Some(now) = sample(state, roi) else {
                        return false;
                    };
                    let changed = last
                        .as_ref()
                        .is_some_and(|l| changed_fraction(&now, l).unwrap_or(1.0) > *threshold);
                    *last = Some(now);
                    changed
                }
                _ => false,
            },
            Matcher::InputSent => matches!(
                &record.event,
                Event::InputSent { source, .. } if *source != InputSource::Trigger
            ),
            Matcher::WindowMapped(m) => match &record.event {
                Event::WindowMapped { window, .. } => state
                    .windows
                    .get(*window)
                    .is_some_and(|rec| m.matches(&state.window_info(rec))),
                _ => false,
            },
            Matcher::AppMark(name) => match &record.event {
                Event::AppMark { mark, .. } => mark_name(mark) == Some(name),
                _ => false,
            },
            Matcher::Vblank {
                every,
                output,
                seen,
            } => match &record.event {
                Event::Vblank {
                    output: o, dropped, ..
                } if !*dropped && Some(*o) == state.outputs.resolve(*output) => {
                    *seen += 1;
                    *seen % *every == 0
                }
                _ => false,
            },
        }
    }
}

fn surface_matches(state: &State, surface: SurfaceId, matcher: Option<&WindowMatcher>) -> bool {
    let Some(m) = matcher else {
        return true;
    };
    state
        .window_of_surface_id(surface)
        .and_then(|w| state.windows.get(w))
        .is_some_and(|rec| m.matches(&state.window_info(rec)))
}

fn sample(state: &mut State, roi: &Roi) -> Option<Image> {
    state
        .capture(
            &CaptureTarget::Roi { roi: roi.clone() },
            CaptureOptions::default(),
        )
        .ok()
        .map(|s| s.image)
}
