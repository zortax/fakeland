//! Reusable offscreen targets. Captures of one size render into the same buffer, so a capture
//! allocates no GPU memory once its size is known. Each capture redraws its whole target.

use smithay::backend::renderer::gles::GlesRenderbuffer;
use smithay::reexports::pixman;
use smithay::utils::{Physical, Size};

/// The number of target sizes that stay allocated.
const KEEP: usize = 3;

/// Offscreen targets of one renderer, keyed by size. The most recently used target is last.
pub struct Targets<T> {
    entries: Vec<(Size<i32, Physical>, T)>,
}

impl<T> Default for Targets<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T> std::fmt::Debug for Targets<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sizes: Vec<_> = self.entries.iter().map(|(s, _)| (s.w, s.h)).collect();
        f.debug_struct("Targets").field("sizes", &sizes).finish()
    }
}

impl<T> Targets<T> {
    /// Takes the target of a size out of the cache, or creates one.
    pub fn take<E>(
        &mut self,
        size: Size<i32, Physical>,
        create: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        match self.entries.iter().position(|(s, _)| *s == size) {
            Some(i) => Ok(self.entries.remove(i).1),
            None => create(),
        }
    }

    /// Returns a target to the cache. The least recently used target goes when the cache is full.
    pub fn put(&mut self, size: Size<i32, Physical>, target: T) {
        if self.entries.len() >= KEEP {
            self.entries.remove(0);
        }
        self.entries.push((size, target));
    }
}

/// The capture targets of both renderers.
#[derive(Debug, Default)]
pub struct CaptureTargets {
    /// Targets of the GLES renderer.
    pub gles: Targets<GlesRenderbuffer>,
    /// Targets of the Pixman renderer.
    pub pixman: Targets<pixman::Image<'static, 'static>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(w: i32) -> Size<i32, Physical> {
        (w, 10).into()
    }

    #[test]
    fn a_target_is_reused_by_size() {
        let mut t = Targets::<u32>::default();
        let made = t.take(size(1), || Ok::<_, ()>(1)).unwrap();
        t.put(size(1), made);
        assert_eq!(t.take(size(1), || Ok::<_, ()>(2)), Ok(1));
        assert_eq!(t.take(size(1), || Ok::<_, ()>(3)), Ok(3));
    }

    #[test]
    fn the_least_recently_used_target_goes_first() {
        let mut t = Targets::<i32>::default();
        for w in 0..=KEEP as i32 {
            t.put(size(w), w);
        }
        assert_eq!(t.take(size(0), || Ok::<_, ()>(-1)), Ok(-1));
        assert_eq!(t.take(size(1), || Ok::<_, ()>(-1)), Ok(1));
    }
}
