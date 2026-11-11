//! The Chrome Trace Event format: one JSON object per event.

use serde::Serialize;
use serde_json::{Map, Value};

/// One Chrome trace event. Times are microseconds since the trace base.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ChromeEvent {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cat: Option<&'static str>,
    pub(crate) ph: &'static str,
    pub(crate) ts: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dur: Option<f64>,
    pub(crate) pid: u64,
    pub(crate) tid: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) s: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bp: Option<&'static str>,
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub(crate) args: Map<String, Value>,
}

impl ChromeEvent {
    fn base(name: impl Into<String>, ph: &'static str, ts: f64, pid: u64, tid: u64) -> Self {
        Self {
            name: name.into(),
            cat: None,
            ph,
            ts,
            dur: None,
            pid,
            tid,
            s: None,
            id: None,
            bp: None,
            args: Map::new(),
        }
    }

    /// A complete slice from `ts` for `dur` microseconds.
    pub(crate) fn complete(
        name: impl Into<String>,
        cat: &'static str,
        ts: f64,
        dur: f64,
        pid: u64,
        tid: u64,
    ) -> Self {
        let mut e = Self::base(name, "X", ts, pid, tid);
        e.cat = Some(cat);
        e.dur = Some(dur.max(0.0));
        e
    }

    /// An instant on a thread.
    pub(crate) fn instant(
        name: impl Into<String>,
        cat: &'static str,
        ts: f64,
        pid: u64,
        tid: u64,
    ) -> Self {
        let mut e = Self::base(name, "i", ts, pid, tid);
        e.cat = Some(cat);
        e.s = Some("t");
        e
    }

    /// An instant across the whole trace.
    pub(crate) fn global_instant(name: impl Into<String>, ts: f64, pid: u64) -> Self {
        let mut e = Self::base(name, "i", ts, pid, 0);
        e.cat = Some("mark");
        e.s = Some("g");
        e
    }

    /// A counter sample with one series.
    pub(crate) fn counter(name: impl Into<String>, ts: f64, pid: u64, value: f64) -> Self {
        let mut e = Self::base(name, "C", ts, pid, 0);
        e.args.insert("value".into(), Value::from(value));
        e
    }

    /// A metadata event, for example `process_name` or `thread_name`.
    pub(crate) fn metadata(name: &'static str, pid: u64, tid: u64, key: &str, v: Value) -> Self {
        let mut e = Self::base(name, "M", 0.0, pid, tid);
        e.args.insert(key.into(), v);
        e
    }

    /// A flow event: `s` starts, `t` steps and `f` ends a flow.
    pub(crate) fn flow(ph: &'static str, id: u64, ts: f64, pid: u64, tid: u64) -> Self {
        let mut e = Self::base("input", ph, ts, pid, tid);
        e.cat = Some("input");
        e.id = Some(id);
        if ph == "f" {
            e.bp = Some("e");
        }
        e
    }

    /// Adds an argument.
    pub(crate) fn arg(mut self, key: &str, v: impl Into<Value>) -> Self {
        self.args.insert(key.into(), v.into());
        self
    }

    /// Adds an argument when it has a value.
    pub(crate) fn opt_arg(self, key: &str, v: Option<impl Into<Value>>) -> Self {
        match v {
            Some(v) => self.arg(key, v),
            None => self,
        }
    }
}
