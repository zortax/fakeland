//! The analyzers. Each one reads an [`AnalysisInput`](crate::AnalysisInput) and returns a typed,
//! serializable report with metrics.

pub mod buffers;
pub mod coalescing;
pub mod drag;
pub mod frame_cost;
pub mod idle;
pub mod latency;
pub mod pacing;
pub mod popup;
pub mod resize;
pub mod resources;
pub mod scroll;
pub mod startup;
pub mod throughput;
pub mod typing;
pub mod util;

pub use buffers::Buffers;
pub use coalescing::InputCoalescing;
pub use drag::DragFollow;
pub use frame_cost::FrameCost;
pub use idle::IdleEfficiency;
pub use latency::Latency;
pub use pacing::FramePacing;
pub use popup::PopupOpen;
pub use resize::Resize;
pub use resources::Resources;
pub use scroll::ScrollSmoothness;
pub use startup::Startup;
pub use throughput::Throughput;
pub use typing::Typing;
