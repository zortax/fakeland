//! What a finished action reports.

use crate::capture::Screenshot;
use crate::input::SentInput;
use crate::probes::ProbeReport;
use crate::server::State;
use crate::waits::WaitOutput;

use super::ActionError;

/// The result of one action.
#[derive(Clone, Debug)]
pub enum ActionOutcome {
    /// The action finished without a value.
    Done,
    /// The input events that reached clients.
    Input {
        /// The events in send order.
        sent: Vec<SentInput>,
    },
    /// A screenshot.
    Captured {
        /// The label of the capture action.
        label: Option<String>,
        /// The screenshot.
        screenshot: Screenshot,
    },
    /// The report of a probe.
    Probe(ProbeReport),
    /// The result of a wait.
    Waited(WaitOutput),
}

/// The callback of an action. It runs once, possibly before `perform` returns.
pub type ActionDone = Box<dyn FnOnce(&mut State, Result<ActionOutcome, ActionError>)>;
