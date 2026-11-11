//! The scenes the client draws and their input state machines: hover, click, scroll, type,
//! animate, resize and idle.

mod animate;
mod click;
mod hover;
mod options;
mod resize;
mod scroll;
mod set;
mod typing;

pub use animate::AnimateScene;
pub use click::ClickScene;
pub use hover::HoverScene;
pub use options::{SceneKind, SceneOptions};
pub use resize::ResizeScene;
pub use scroll::{ScrollScene, ScrollUnit};
pub use set::{DrawContext, FrameState, SceneSet};
pub use typing::TypeScene;
