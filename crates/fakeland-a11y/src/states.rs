//! AT-SPI state sets: the two 32-bit words of `GetState` as state names. Bits that this crate does
//! not know read `state-<bit>`.

use atspi::StateSet;

/// Returns the names of the set bits of a state set.
pub fn state_names(words: &[u32]) -> Vec<String> {
    let lo = words.first().copied().unwrap_or(0) as u64;
    let hi = words.get(1).copied().unwrap_or(0) as u64;
    let bits = lo | (hi << 32);
    (0..64)
        .filter(|i| bits & (1u64 << i) != 0)
        .map(|i| match StateSet::from_bits(1u64 << i) {
            Ok(set) => set
                .iter()
                .next()
                .map(|s| s.to_static_str().to_string())
                .unwrap_or_else(|| format!("state-{i}")),
            Err(_) => format!("state-{i}"),
        })
        .collect()
}

/// Returns true when a state list holds a state.
pub fn has_state(states: &[String], name: &str) -> bool {
    states.iter().any(|s| s == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_atspi_bit_order() {
        // Bit 1 is active, bit 8 is enabled, bit 12 is focused, bit 25 is showing.
        let words = [(1 << 1) | (1 << 8) | (1 << 12) | (1 << 25), 0];
        assert_eq!(
            state_names(&words),
            vec!["active", "enabled", "focused", "showing"]
        );
    }

    #[test]
    fn unknown_bits_keep_their_index() {
        let names = state_names(&[0, 1 << 30]);
        assert_eq!(names, vec!["state-62"]);
    }

    #[test]
    fn short_words_read_as_zero() {
        assert!(state_names(&[]).is_empty());
        assert_eq!(state_names(&[1 << 4]), vec!["checked"]);
    }
}
