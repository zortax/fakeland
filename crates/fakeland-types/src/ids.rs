//! Typed ids. Each id is a plain integer newtype with a distinct type per domain.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

macro_rules! id {
    ($(#[$doc:meta])* $name:ident($inner:ty), $prefix:literal) => {
        $(#[$doc])*
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
        )]
        #[serde(transparent)]
        pub struct $name(pub $inner);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "{}"), self.0)
            }
        }
    };
}

id!(
    /// A Wayland surface, unique for the life of a session.
    SurfaceId(u64), "surface#"
);
id!(
    /// One `wl_surface.commit` of a surface, unique for the life of a session.
    CommitId(u64), "commit#"
);
id!(
    /// A connected Wayland client.
    ClientId(u64), "client#"
);
id!(
    /// A faked output.
    OutputId(u32), "output#"
);
id!(
    /// A mapped toplevel window.
    WindowId(u64), "window#"
);
id!(
    /// One injected input event.
    InputEventId(u64), "input#"
);
id!(
    /// One latency probe.
    ProbeId(u64), "probe#"
);
id!(
    /// One scenario run.
    RunId(u64), "run#"
);
id!(
    /// A client buffer.
    BufferId(u64), "buffer#"
);
id!(
    /// An operating system process id.
    ProcessId(u32), "pid "
);
