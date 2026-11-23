//! Layer-shell surfaces: the registry, the lifecycle from creation to close, the layout of each
//! output with its exclusive zones, the stacking order around the toplevels and the keyboard
//! focus rules of layer, lock and toplevel surfaces.

mod arrange;
mod convert;
mod focus;
mod lifecycle;
mod registry;
mod scene;
mod stack;

pub use convert::layer_info;
pub use registry::{LayerRecord, Layers};
pub use stack::Hit;
