//! App-side instrumentation for the fakeland_marks_v1 protocol. An app marks app frames, handled
//! input events, client-side phases and named marks on the commits of its surfaces, and the
//! compositor records them in its trace. The crate works with any wayland-client app and with
//! toolkits that run their own event loop: it dispatches its objects on a private event queue.
//! Every call is a cheap no-op when the compositor advertises no fakeland_marks_manager_v1 global.
//!
//! ```no_run
//! use fakeland_marks::{Marks, Span};
//! use wayland_client::Connection;
//! use wayland_client::protocol::wl_surface::WlSurface;
//!
//! fn frame(conn: &Connection, marks: &mut Marks, surface: &WlSurface, frame_id: u64) {
//!     let surface_marks = marks.surface(surface).expect("one marks object per surface");
//!     // The app dispatched its input events. Each id belongs to an event it handled.
//!     for id in marks.take_input_ids() {
//!         surface_marks.input_handled(id);
//!     }
//!     surface_marks.frame(frame_id);
//!     let paint = Span::start("paint");
//!     // The app paints here.
//!     paint.finish(&surface_marks).expect("valid span");
//!     surface.commit();
//!     conn.flush().ok();
//! }
//!
//! let conn = Connection::connect_to_env().expect("wayland connection");
//! let marks = Marks::discover(&conn).expect("registry roundtrip");
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod bind;
mod check;
mod claims;
mod family;
mod input;
mod marks;
mod queue;
mod span;
mod surface;

pub use bind::DiscoverError;
pub use check::MarkError;
pub use fakeland_protocol::wire::{MAX_NAME_LEN, NameError};
pub use fakeland_protocol::{MANAGER_INTERFACE, MANAGER_VERSION};
pub use fakeland_types::{InputEventId, Timestamp};
pub use family::Family;
pub use input::{InputKind, InputMark};
pub use marks::Marks;
pub use span::Span;
pub use surface::{SurfaceError, SurfaceMarks};
