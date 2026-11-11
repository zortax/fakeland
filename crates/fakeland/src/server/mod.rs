//! The compositor thread and its state.
//!
//! `State` owns everything: `protocols` (every smithay protocol state), `seat`, `space` and
//! `popups`, `outputs` (faked outputs with their schedulers), `surfaces` and `frames` (the commit
//! pipeline), `windows`, `clients`, `processes`, `timers` with `timer` (the deadline queue),
//! `bus` (trace recorder and subscribers), `observers` and `waiters`, `render`, `shm_reader` and
//! `lateness`, `input` (the injector) and `text_inputs`, `marks` (app-mark managers),
//! `frame_captures`, `runs` (scenario runs) and `reactors`.
//!
//! Extension points on `State`: `schedule_at`/`schedule_in`/`cancel_timer` run closures at
//! deadlines; `observe`/`unobserve` see every trace record after each dispatch; `add_waiter`
//! polls a condition with a deadline; `emit` records an event; `capture` renders and reads back a
//! target; `flush_clients` sends queued events at once; `inject_batch` injects primitives;
//! `perform` runs one scenario action; `start_wait`, `start_probe` and `start_run` run waits,
//! probes and scenarios on the thread.

mod client;
mod ids;
mod lifecycle;
mod run;
mod state;

pub use client::{ClientInfo, ClientState, Clients};
pub use ids::Ids;
pub use run::{KILL_GRACE, spawn};
pub use state::State;
