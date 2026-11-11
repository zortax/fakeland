//! CSV fields: RFC 4180 quoting and the text form of optional values.

use std::fmt::Display;

use fakeland_types::{Nanos, Timestamp};

/// One CSV row under construction.
#[derive(Default)]
pub(crate) struct Row {
    fields: Vec<String>,
}

impl Row {
    /// Appends a value.
    pub(crate) fn push(&mut self, v: impl Display) -> &mut Self {
        self.fields.push(v.to_string());
        self
    }

    /// Appends an optional value. `None` is an empty field.
    pub(crate) fn opt(&mut self, v: Option<impl Display>) -> &mut Self {
        self.fields
            .push(v.map(|v| v.to_string()).unwrap_or_default());
        self
    }

    /// Appends an optional timestamp in nanoseconds.
    pub(crate) fn ts(&mut self, v: Option<Timestamp>) -> &mut Self {
        self.opt(v.map(Timestamp::as_nanos))
    }

    /// Appends an optional span in milliseconds with nanosecond precision.
    pub(crate) fn ms(&mut self, v: Option<Nanos>) -> &mut Self {
        self.opt(v.map(|n| format!("{:.6}", n.as_millis_f64())))
    }

    /// Returns the row as one CSV line with a trailing newline.
    pub(crate) fn line(&self) -> String {
        let mut out = self
            .fields
            .iter()
            .map(|f| quote(f))
            .collect::<Vec<_>>()
            .join(",");
        out.push('\n');
        out
    }
}

/// Quotes a field that holds a comma, a quote or a line break.
pub(crate) fn quote(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(quote("plain"), "plain");
        assert_eq!(quote("a,b"), "\"a,b\"");
        assert_eq!(quote("say \"hi\""), "\"say \"\"hi\"\"\"");
        let mut r = Row::default();
        r.push(1)
            .opt(None::<u32>)
            .ms(Some(Nanos(1_500_000)))
            .ts(Some(Timestamp(9)));
        assert_eq!(r.line(), "1,,1.500000,9\n");
    }
}
