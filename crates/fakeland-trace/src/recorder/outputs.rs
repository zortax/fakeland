//! The output list at a point of a trace, from the header outputs and the context records.

use crate::event::{Event, OutputInfo, Record};

/// Returns `initial` updated by the output events in `context`.
pub(crate) fn outputs_at(initial: &[OutputInfo], context: &[Record]) -> Vec<OutputInfo> {
    let mut outputs = initial.to_vec();
    for r in context {
        match &r.event {
            Event::OutputConfigured(info) | Event::OutputChanged(info) => {
                match outputs.iter_mut().find(|o| o.output == info.output) {
                    Some(o) => *o = info.clone(),
                    None => outputs.push(info.clone()),
                }
            }
            Event::OutputRemoved { output } => outputs.retain(|o| o.output != *output),
            _ => {}
        }
    }
    outputs
}
