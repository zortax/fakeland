//! Presentation feedback flags with the `wp_presentation_feedback.kind` values.

use std::fmt;
use std::ops::{BitOr, BitOrAssign};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A set of presentation feedback flags.
#[derive(
    Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct PresentFlags(pub u32);

impl PresentFlags {
    /// No flags.
    pub const EMPTY: Self = Self(0);
    /// The presentation was synchronized to the vertical retrace.
    pub const VSYNC: Self = Self(1);
    /// The timestamp comes from the display hardware clock.
    pub const HW_CLOCK: Self = Self(2);
    /// The display hardware signaled the completion.
    pub const HW_COMPLETION: Self = Self(4);
    /// The client buffer was scanned out directly.
    pub const ZERO_COPY: Self = Self(8);

    /// Returns the raw bits.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns true when every flag of `other` is set.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns true when no flag is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns the union of two sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOr for PresentFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl BitOrAssign for PresentFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

impl fmt::Debug for PresentFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names = [
            (Self::VSYNC, "VSYNC"),
            (Self::HW_CLOCK, "HW_CLOCK"),
            (Self::HW_COMPLETION, "HW_COMPLETION"),
            (Self::ZERO_COPY, "ZERO_COPY"),
        ];
        let set: Vec<&str> = names
            .iter()
            .filter(|(flag, _)| self.contains(*flag))
            .map(|(_, name)| *name)
            .collect();
        write!(f, "PresentFlags({})", set.join("|"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_values_match_the_protocol() {
        let all = PresentFlags::VSYNC
            | PresentFlags::HW_CLOCK
            | PresentFlags::HW_COMPLETION
            | PresentFlags::ZERO_COPY;
        assert_eq!(all.bits(), 15);
        assert!(all.contains(PresentFlags::HW_CLOCK | PresentFlags::VSYNC));
        assert!(!PresentFlags::VSYNC.contains(PresentFlags::ZERO_COPY));
        assert!(PresentFlags::EMPTY.is_empty());
        assert_eq!(
            format!("{:?}", PresentFlags(5)),
            "PresentFlags(VSYNC|HW_COMPLETION)"
        );
    }
}
