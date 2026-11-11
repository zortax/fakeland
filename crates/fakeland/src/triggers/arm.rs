//! Arming and firing the triggers of a run. One observer per run reads every record; a match
//! logs the firing and runs the trigger actions in order, after the trigger delay.

use std::collections::VecDeque;

use fakeland_scenario::{Action, Trigger};
use fakeland_trace::{InputSource, Record};
use fakeland_types::RunId;

use super::{Armed, TriggerSet};
use crate::actions::ActionCtx;
use crate::runner::TriggerFiring;
use crate::server::State;
use crate::util::CancelToken;
use crate::waits::Observe;

impl State {
    /// Arms the triggers of a run.
    pub fn arm_triggers(&mut self, run: RunId, triggers: &[Trigger]) {
        if triggers.is_empty() {
            return;
        }
        let mut set = TriggerSet::default();
        for (i, t) in triggers.iter().enumerate() {
            match Armed::new(self, i, t) {
                Ok(a) => set.list.push(a),
                Err(e) => tracing::warn!("trigger {i} not armed: {e}"),
            }
        }
        let observer = self.observe(move |state, record| {
            if state.runs.active.contains_key(&run) {
                state.run_triggers(run, record);
                Observe::Keep
            } else {
                Observe::Remove
            }
        });
        if let Some(r) = self.runs.active.get_mut(&run) {
            r.triggers = Some(set);
            r.trigger_observer = Some(observer);
        }
    }

    fn run_triggers(&mut self, run: RunId, record: &Record) {
        let Some(mut set) = self
            .runs
            .active
            .get_mut(&run)
            .and_then(|r| r.triggers.take())
        else {
            return;
        };
        let token = match self.runs.active.get(&run) {
            Some(r) => r.token.clone(),
            None => return,
        };
        let mut fired = Vec::new();
        for armed in &mut set.list {
            if armed.matches(self, record) {
                armed.fired += 1;
                fired.push((armed.index, armed.trigger.clone(), armed.fired));
            }
        }
        if let Some(r) = self.runs.active.get_mut(&run) {
            r.triggers = Some(set);
        }
        for (index, trigger, count) in fired {
            let now = self.now();
            let log = TriggerFiring {
                trigger: index,
                label: trigger.label.clone(),
                event: record.event.name().to_string(),
                event_time: record.t,
                fired: now,
                count,
                error: None,
            };
            let slot = match self.runs.active.get_mut(&run) {
                Some(r) => {
                    r.trigger_log.push(log);
                    r.trigger_log.len() - 1
                }
                None => return,
            };
            let actions: VecDeque<Action> = trigger.then.iter().cloned().collect();
            let token = token.clone();
            match trigger.delay {
                Some(d) if d.0.as_nanos() > 0 => {
                    self.schedule_in(d.0, move |state| {
                        if !token.is_cancelled() {
                            if let Some(r) = state.runs.active.get_mut(&run)
                                && let Some(entry) = r.trigger_log.get_mut(slot)
                            {
                                entry.fired = state.clock.now();
                            }
                            run_chain(state, run, slot, actions, token);
                        }
                    });
                }
                _ => run_chain(self, run, slot, actions, token),
            }
        }
    }
}

/// Runs trigger actions in order. The first failure lands in the trigger log.
fn run_chain(
    state: &mut State,
    run: RunId,
    slot: usize,
    mut actions: VecDeque<Action>,
    token: CancelToken,
) {
    let Some(action) = actions.pop_front() else {
        return;
    };
    let ctx = ActionCtx::new(InputSource::Trigger).with_token(token.clone());
    state.perform(
        &action,
        ctx,
        Box::new(move |state, result| match result {
            Ok(_) => run_chain(state, run, slot, actions, token),
            Err(e) => {
                if let Some(r) = state.runs.active.get_mut(&run)
                    && let Some(entry) = r.trigger_log.get_mut(slot)
                {
                    entry.error = Some(e.to_string());
                }
            }
        }),
    );
}
