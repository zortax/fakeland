//! The typed description of a method: its wire name, parameters and result.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// A control method. The client serializes `Params`, the server answers with `Output`.
pub trait Method {
    /// The wire name.
    const NAME: &'static str;
    /// The parameters.
    type Params: Serialize + DeserializeOwned + Send + 'static;
    /// The result.
    type Output: Serialize + DeserializeOwned + Send + 'static;
}

/// The parameters of a method that takes none. JSON `null` and `{}` both read as this value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoParams {}

/// Defines a marker type for a method and implements [`Method`] for it.
macro_rules! method {
    ($(#[$doc:meta])* $ty:ident, $name:literal, $params:ty => $output:ty) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, Default)]
        pub struct $ty;

        impl $crate::methods::Method for $ty {
            const NAME: &'static str = $name;
            type Params = $params;
            type Output = $output;
        }
    };
}

pub(crate) use method;
