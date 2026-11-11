//! The trace format version. Readers accept only traces of the same version.

/// The version of the trace event format. A breaking change to any event bumps it.
pub const TRACE_FORMAT_VERSION: u32 = 1;
