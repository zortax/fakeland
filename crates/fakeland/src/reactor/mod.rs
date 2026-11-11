//! Reactors: Rust code that runs on the compositor thread and sees every trace record as it
//! happens. A reactor reacts with zero round trip: it injects input now or at an exact time,
//! runs scenario actions, asks for region captures of the next frame, reads the vblank grid and
//! sends values back to the caller. This is the path for benches whose logic does not fit a
//! declarative scenario.
//!
//! ```no_run
//! use fakeland::{Event, Fakeland, ReactorCx, Record, reactor_fn};
//!
//! let session = Fakeland::builder().start()?;
//! let handle = session.blocking_install_reactor(reactor_fn(
//!     |record: &Record, cx: &mut ReactorCx<'_, u64>| {
//!         if let Event::Presented { seq, .. } = record.event {
//!             cx.emit(seq);
//!             if seq > 100 {
//!                 cx.finish();
//!             }
//!         }
//!     },
//! ))?;
//! while let Some(seq) = handle.blocking_recv() {
//!     println!("presented at vblank {seq}");
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod cx;
mod handle;
mod install;
mod traits;

pub use cx::ReactorCx;
pub use handle::{ReactorHandle, ReactorId};
pub use install::Reactors;
pub use traits::{FnReactor, Reactor, reactor_fn};
