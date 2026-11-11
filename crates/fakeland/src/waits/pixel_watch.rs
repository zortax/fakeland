//! Pixel waits: a region pinned to a window or an output, sampled at vblanks after content
//! changes, and tested for stability or for a condition.

use fakeland_scenario::{CaptureTarget, Roi};
use fakeland_trace::{Event, Record};
use fakeland_types::{OutputId, Timestamp};
use fakeland_vision::{Image, StabilityConfig, StabilityTracker, region_hash};

use super::pixels::Condition;
use crate::capture::CaptureOptions;
use crate::server::State;

use super::start::{WaitError, WaitOutput};

/// What a pixel wait waits for.
pub(super) enum PixelGoal {
    /// The region keeps its pixels for this many vblanks.
    Stable(u32),
    /// The region matches a condition.
    Match(Condition),
}

/// The state of a pixel wait: the region, the last sample and whether content changed since.
pub(super) struct PixelWatch {
    roi: Roi,
    output: OutputId,
    goal: PixelGoal,
    tracker: StabilityTracker,
    last_hash: u64,
    baseline: Option<Image>,
    matched: bool,
    stable: bool,
    dirty: bool,
}

impl PixelWatch {
    pub(super) fn start(
        state: &mut State,
        roi: Roi,
        output: OutputId,
        goal: PixelGoal,
    ) -> Result<Self, WaitError> {
        let frames = match &goal {
            PixelGoal::Stable(n) => (*n).max(1),
            PixelGoal::Match(_) => 1,
        };
        let mut watch = Self {
            roi,
            output,
            goal,
            tracker: StabilityTracker::new(StabilityConfig {
                frames,
                ..StabilityConfig::default()
            }),
            last_hash: 0,
            baseline: None,
            matched: false,
            stable: false,
            dirty: false,
        };
        watch.sample(state)?;
        Ok(watch)
    }

    pub(super) fn sample(&mut self, state: &mut State) -> Result<(), WaitError> {
        let shot = state.capture(
            &CaptureTarget::Roi {
                roi: self.roi.clone(),
            },
            CaptureOptions::default(),
        )?;
        let px_per_logical = 1.0 / shot.scale;
        match &self.goal {
            PixelGoal::Stable(_) => {
                self.last_hash = region_hash(&shot.image, None);
                self.stable = self.tracker.push_hash(self.last_hash).stable;
            }
            PixelGoal::Match(cond) => {
                if cond.needs_baseline() && self.baseline.is_none() {
                    self.baseline = Some(shot.image);
                    return Ok(());
                }
                self.matched = cond.matches(&shot.image, self.baseline.as_ref(), px_per_logical)?;
            }
        }
        Ok(())
    }

    pub(super) fn result(&self, now: Timestamp) -> Option<Result<WaitOutput, WaitError>> {
        (self.stable || self.matched).then(|| Ok(WaitOutput::at(now)))
    }

    pub(super) fn on_record(
        &mut self,
        state: &mut State,
        record: &Record,
    ) -> Option<Result<WaitOutput, WaitError>> {
        match &record.event {
            Event::Latched { .. } | Event::WindowMapped { .. } | Event::WindowUnmapped { .. } => {
                self.dirty = true;
                None
            }
            Event::Vblank {
                output, dropped, ..
            } if *output == self.output && !*dropped => {
                if self.dirty {
                    self.dirty = false;
                    if let Err(e) = self.sample(state) {
                        return Some(Err(e));
                    }
                } else if matches!(self.goal, PixelGoal::Stable(_)) {
                    self.stable = self.tracker.push_hash(self.last_hash).stable;
                }
                self.result(record.t)
            }
            _ => None,
        }
    }
}
