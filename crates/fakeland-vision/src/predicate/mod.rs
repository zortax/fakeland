//! Pixel predicates on frames: a color at a point, the mean color of a region, a change against a
//! baseline and a match against a reference image. Predicates are serde values; reference images
//! are named by handles that an image source resolves.

mod error;
mod evaluate;
mod kinds;
mod store;

pub use error::PredicateError;
pub use evaluate::{Evaluation, evaluate};
pub use kinds::{Predicate, Rgba};
pub use store::{ImageHandle, ImageSource, ImageStore, NoImages};
