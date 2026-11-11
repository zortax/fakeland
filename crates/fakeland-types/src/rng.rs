//! A small seeded random number generator with a stable output stream. Seeded perturbation,
//! typing jitter and randomized input phases use it, so a seed reproduces a run on every build.

/// The SplitMix64 generator. The stream for a seed never changes between versions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitMix64 {
    state: u64,
}

const GOLDEN: u64 = 0x9e37_79b9_7f4a_7c15;

impl SplitMix64 {
    /// Builds a generator from a seed.
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN);
        finalize(self.state)
    }

    /// Returns a uniform value in `0.0..1.0`.
    pub fn next_f64(&mut self) -> f64 {
        unit_f64(self.next_u64())
    }

    /// Returns a uniform value in `-1.0..1.0`.
    pub fn next_signed_f64(&mut self) -> f64 {
        self.next_f64() * 2.0 - 1.0
    }

    /// Returns a uniform integer in `0..n`. Returns 0 when `n` is 0.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        ((u128::from(self.next_u64()) * u128::from(n)) >> 64) as u64
    }

    /// Returns 64 random bits for an index in a seeded stream, without generator state. The same
    /// seed, salt and index always give the same value.
    pub fn hash(seed: u64, salt: u64, index: u64) -> u64 {
        let mixed = finalize(seed ^ finalize(salt.wrapping_add(GOLDEN)));
        finalize(mixed.wrapping_add(index.wrapping_mul(GOLDEN)))
    }

    /// Returns a uniform value in `0.0..1.0` for an index in a seeded stream.
    pub fn hash_f64(seed: u64, salt: u64, index: u64) -> f64 {
        unit_f64(Self::hash(seed, salt, index))
    }
}

fn finalize(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn unit_f64(bits: u64) -> f64 {
    (bits >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_is_stable() {
        let mut r = SplitMix64::new(1234567);
        assert_eq!(r.next_u64(), 6457827717110365317);
        assert_eq!(r.next_u64(), 3203168211198807973);
    }

    #[test]
    fn floats_stay_in_range() {
        let mut r = SplitMix64::new(7);
        for _ in 0..10_000 {
            let f = r.next_f64();
            assert!((0.0..1.0).contains(&f));
            let s = r.next_signed_f64();
            assert!((-1.0..1.0).contains(&s));
            assert!(r.below(10) < 10);
        }
        assert_eq!(r.below(0), 0);
    }

    #[test]
    fn hash_depends_on_every_input() {
        let a = SplitMix64::hash(1, 2, 3);
        assert_eq!(a, SplitMix64::hash(1, 2, 3));
        assert_ne!(a, SplitMix64::hash(0, 2, 3));
        assert_ne!(a, SplitMix64::hash(1, 0, 3));
        assert_ne!(a, SplitMix64::hash(1, 2, 0));
        assert!((0.0..1.0).contains(&SplitMix64::hash_f64(1, 2, 3)));
    }
}
