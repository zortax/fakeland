//! Waits as compositor-side futures. A wait polls state, watches trace records or samples a
//! region at each vblank, and reports once: when its condition holds, at its deadline, or never
//! after its token is cancelled.

use std::cell::Cell;
use std::rc::Rc;

use fakeland_scenario::{Roi, Wait, WindowRef};
use fakeland_trace::{AppMarkKind, Event, Record};
use fakeland_types::{CommitId, Timestamp, WindowId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::Observe;
use super::pixel_watch::{PixelGoal, PixelWatch};
use super::pixels::{Condition, ConditionError};
use crate::capture::CaptureError;
use crate::display::TimerId;
use crate::server::State;
use crate::shell::{MatchError, WindowInfo, WindowMatcher};
use crate::util::{CancelToken, TakeOnce};
use crate::waits::ObserverId;

/// What a finished wait reports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WaitOutput {
    /// The time the condition held.
    pub time: Timestamp,
    /// The window the wait matched.
    pub window: Option<WindowInfo>,
    /// The commit that satisfied the wait, for mark waits.
    pub commit: Option<CommitId>,
    /// The presentation time of the last frame, for frame waits.
    pub presented: Option<Timestamp>,
}

impl WaitOutput {
    pub(super) fn at(time: Timestamp) -> Self {
        Self {
            time,
            window: None,
            commit: None,
            presented: None,
        }
    }
}

/// A wait that failed.
#[derive(Debug, Error)]
pub enum WaitError {
    /// The deadline passed.
    #[error("the wait timed out")]
    Timeout,
    /// No mapped window matches.
    #[error("no matching window")]
    NoWindow,
    /// The window matcher is invalid.
    #[error(transparent)]
    Match(#[from] MatchError),
    /// The region capture failed.
    #[error(transparent)]
    Capture(#[from] CaptureError),
    /// The pixel condition cannot be evaluated.
    #[error(transparent)]
    Condition(#[from] ConditionError),
}

/// The callback of a wait.
pub type WaitDone = Box<dyn FnOnce(&mut State, Result<WaitOutput, WaitError>)>;

type RecordCheck = Box<dyn FnMut(&mut State, &Record) -> Option<Result<WaitOutput, WaitError>>>;

impl State {
    /// Starts a wait. `done` runs once, possibly before this call returns.
    pub fn start_wait(&mut self, wait: &Wait, token: CancelToken, done: WaitDone) {
        let deadline = self.now() + wait.timeout().0;
        match wait {
            Wait::Window { matcher, .. } => match WindowMatcher::new(matcher) {
                Ok(m) => self.poll_until(deadline, token, done, move |state| {
                    m.select(&state.window_infos()).cloned().map(|w| {
                        Ok(WaitOutput {
                            window: Some(w),
                            ..WaitOutput::at(state.now())
                        })
                    })
                }),
                Err(e) => done(self, Err(e.into())),
            },
            Wait::Settled { window, quiet, .. } => {
                let matcher =
                    match WindowMatcher::new(window.as_ref().unwrap_or(&WindowRef::Focused)) {
                        Ok(m) => m,
                        Err(e) => return done(self, Err(e.into())),
                    };
                let quiet = quiet.0;
                // The quiet span counts from the start of the wait at the earliest, so a commit
                // that answers input sent just before the wait is always seen.
                let started = self.now();
                let mut armed: Option<Timestamp> = None;
                self.poll_until(deadline, token, done, move |state| {
                    let now = state.now();
                    let w = matcher.select(&state.window_infos()).cloned()?;
                    let since = state.quiet_since(w.id)?.max(started);
                    if now.since(since) >= quiet {
                        return Some(Ok(WaitOutput {
                            window: Some(w),
                            ..WaitOutput::at(now)
                        }));
                    }
                    let wake = since + quiet;
                    if armed != Some(wake) {
                        armed = Some(wake);
                        state.schedule_at(wake, |_| {});
                    }
                    None
                });
            }
            Wait::Frames { n, window, .. } => {
                self.wait_presented(*n, window.as_ref(), deadline, token, done)
            }
            Wait::Presented { window, .. } => {
                self.wait_presented(1, window.as_ref(), deadline, token, done)
            }
            Wait::PixelsStable { roi, frames, .. } => {
                self.wait_pixels(roi, PixelGoal::Stable(*frames), deadline, token, done)
            }
            Wait::PixelsMatch { roi, condition, .. } => match Condition::new(condition) {
                Ok(c) => self.wait_pixels(roi, PixelGoal::Match(c), deadline, token, done),
                Err(e) => done(self, Err(e.into())),
            },
            Wait::AppMark { name, .. } => {
                let name = name.clone();
                self.watch_records(
                    deadline,
                    token,
                    done,
                    Box::new(move |_, record| match &record.event {
                        Event::AppMark { commit, mark, .. } if mark_name(mark) == Some(&name) => {
                            Some(Ok(WaitOutput {
                                commit: *commit,
                                ..WaitOutput::at(record.t)
                            }))
                        }
                        _ => None,
                    }),
                );
            }
        }
    }

    /// Polls a condition after every dispatch until it holds or the deadline passes.
    fn poll_until(
        &mut self,
        deadline: Timestamp,
        token: CancelToken,
        done: WaitDone,
        mut check: impl FnMut(&mut State) -> Option<Result<WaitOutput, WaitError>> + 'static,
    ) {
        let mut done = Some(done);
        self.add_waiter(Some(deadline), move |state, timed_out| {
            if token.is_cancelled() {
                return true;
            }
            let result = match check(state) {
                Some(r) => r,
                None if timed_out => Err(WaitError::Timeout),
                None => return false,
            };
            if let Some(d) = done.take() {
                d(state, result);
            }
            true
        });
    }

    /// Watches trace records until a check reports or the deadline passes.
    pub(crate) fn watch_records(
        &mut self,
        deadline: Timestamp,
        token: CancelToken,
        done: WaitDone,
        mut check: RecordCheck,
    ) {
        let done = TakeOnce::new(done);
        let timer: Rc<Cell<Option<TimerId>>> = Rc::new(Cell::new(None));
        let observer: Rc<Cell<Option<ObserverId>>> = Rc::new(Cell::new(None));
        let (obs_done, obs_timer, obs_token) = (done.clone(), timer.clone(), token.clone());
        let id = self.observe(move |state, record| {
            if obs_token.is_cancelled() || !obs_done.is_pending() {
                if let Some(t) = obs_timer.take() {
                    state.cancel_timer(t);
                }
                return Observe::Remove;
            }
            let Some(result) = check(state, record) else {
                return Observe::Keep;
            };
            if let Some(t) = obs_timer.take() {
                state.cancel_timer(t);
            }
            if let Some(d) = obs_done.take() {
                d(state, result);
            }
            Observe::Remove
        });
        observer.set(Some(id));
        let t = self.schedule_at(deadline, move |state| {
            if let Some(o) = observer.take() {
                state.unobserve(o);
            }
            if token.is_cancelled() {
                return;
            }
            if let Some(d) = done.take() {
                d(state, Err(WaitError::Timeout));
            }
        });
        timer.set(Some(t));
    }

    fn wait_presented(
        &mut self,
        n: u32,
        window: Option<&WindowRef>,
        deadline: Timestamp,
        token: CancelToken,
        done: WaitDone,
    ) {
        let target = match window.map(|w| self.resolve_window(w)) {
            None => None,
            Some(Ok(Some(id))) => Some(id),
            Some(Ok(None)) => return done(self, Err(WaitError::NoWindow)),
            Some(Err(e)) => return done(self, Err(e.into())),
        };
        if n == 0 {
            return done(self, Ok(WaitOutput::at(self.now())));
        }
        let mut left = n;
        self.watch_records(
            deadline,
            token,
            done,
            Box::new(move |state, record| {
                let Event::Presented { surface, time, .. } = &record.event else {
                    return None;
                };
                if let Some(w) = target
                    && state.window_of_surface_id(*surface) != Some(w)
                {
                    return None;
                }
                left -= 1;
                (left == 0).then(|| {
                    Ok(WaitOutput {
                        presented: Some(*time),
                        window: target
                            .and_then(|w| state.windows.get(w))
                            .map(|r| state.window_info(r)),
                        ..WaitOutput::at(record.t)
                    })
                })
            }),
        );
    }

    fn wait_pixels(
        &mut self,
        roi: &Roi,
        goal: PixelGoal,
        deadline: Timestamp,
        token: CancelToken,
        done: WaitDone,
    ) {
        let (roi, output) = match self.pin_roi(roi).and_then(|r| {
            let output = self.resolve_roi(&r)?.output;
            Ok((r, output))
        }) {
            Ok(v) => v,
            Err(e) => return done(self, Err(e.into())),
        };
        let mut watch = match PixelWatch::start(self, roi, output, goal) {
            Ok(w) => w,
            Err(e) => return done(self, Err(e)),
        };
        if let Some(result) = watch.result(self.now()) {
            return done(self, result);
        }
        self.watch_records(
            deadline,
            token,
            done,
            Box::new(move |state, record| watch.on_record(state, record)),
        );
    }

    /// Returns the window of a surface by session id.
    pub fn window_of_surface_id(&self, surface: fakeland_types::SurfaceId) -> Option<WindowId> {
        let wl = self.surfaces.get(surface)?.wl.clone();
        self.window_of_surface(&wl)
    }
}

/// Returns the name of a named mark or span.
pub fn mark_name(mark: &AppMarkKind) -> Option<&String> {
    match mark {
        AppMarkKind::Mark { name } | AppMarkKind::Span { name, .. } => Some(name),
        _ => None,
    }
}
