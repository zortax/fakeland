//! 64-bit values on the wire. The protocol sends a 64-bit id as a high and a low 32-bit half.

/// Splits a 64-bit value into its high and low 32-bit halves.
pub const fn split_u64(value: u64) -> (u32, u32) {
    ((value >> 32) as u32, value as u32)
}

/// Joins a high and a low 32-bit half into a 64-bit value.
pub const fn join_u64(hi: u32, lo: u32) -> u64 {
    ((hi as u64) << 32) | lo as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_round_trip() {
        for v in [
            0,
            1,
            u32::MAX as u64,
            u32::MAX as u64 + 1,
            0x0123_4567_89ab_cdef,
            u64::MAX,
        ] {
            let (hi, lo) = split_u64(v);
            assert_eq!(join_u64(hi, lo), v);
        }
    }

    #[test]
    fn high_half_holds_the_upper_bits() {
        assert_eq!(split_u64(0x0000_0002_0000_0003), (2, 3));
        assert_eq!(join_u64(1, 0), 1 << 32);
    }
}
