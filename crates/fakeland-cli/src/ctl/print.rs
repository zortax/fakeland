//! Output of ctl results: JSON with `--json`, text otherwise.

use anyhow::Result;
use fakeland::SentInput;
use serde::Serialize;

use crate::util::print_json;

/// Prints a value as JSON, or its text form.
pub fn emit<T: Serialize>(json: bool, value: &T, text: impl FnOnce(&T) -> String) -> Result<()> {
    if json {
        print_json(value)
    } else {
        let t = text(value);
        if t.ends_with('\n') {
            print!("{t}");
        } else if !t.is_empty() {
            println!("{t}");
        }
        Ok(())
    }
}

/// Describes sent input events.
pub fn sent_text(sent: &[SentInput]) -> String {
    match (sent.first(), sent.last()) {
        (Some(first), Some(last)) => format!(
            "sent {} events over {:.3} ms",
            sent.len(),
            last.time.since(first.time).as_millis_f64()
        ),
        _ => "sent no events: no client has focus there".to_owned(),
    }
}
