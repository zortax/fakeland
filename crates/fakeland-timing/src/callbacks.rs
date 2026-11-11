//! Armed frame callbacks: each waits for the first dispatch at or after its eligible time.

use fakeland_types::{CommitId, SurfaceId, Timestamp};

use crate::FrameCallback;

#[derive(Clone, Copy, Debug)]
struct Armed {
    callback: FrameCallback,
    eligible: Timestamp,
}

/// The frame callbacks that wait for a dispatch.
#[derive(Clone, Debug, Default)]
pub(crate) struct CallbackBook {
    armed: Vec<Armed>,
}

impl CallbackBook {
    /// Arms a callback that fires at the first dispatch at or after `eligible`.
    pub(crate) fn arm(&mut self, surface: SurfaceId, commit: CommitId, eligible: Timestamp) {
        self.armed.push(Armed {
            callback: FrameCallback { surface, commit },
            eligible,
        });
    }

    /// Returns the earliest eligible time.
    pub(crate) fn earliest(&self) -> Option<Timestamp> {
        self.armed.iter().map(|a| a.eligible).min()
    }

    /// Removes and returns every callback that is eligible at `at`, sorted.
    pub(crate) fn take_due(&mut self, at: Timestamp) -> Vec<FrameCallback> {
        let mut due = Vec::new();
        self.armed.retain(|a| {
            let fire = a.eligible <= at;
            if fire {
                due.push(a.callback);
            }
            !fire
        });
        due.sort();
        due
    }

    /// Drops every callback of a surface.
    pub(crate) fn remove_surface(&mut self, surface: SurfaceId) {
        self.armed.retain(|a| a.callback.surface != surface);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_callbacks_leave_the_book_sorted() {
        let mut b = CallbackBook::default();
        b.arm(SurfaceId(2), CommitId(5), Timestamp(10));
        b.arm(SurfaceId(1), CommitId(4), Timestamp(10));
        b.arm(SurfaceId(1), CommitId(6), Timestamp(30));
        assert_eq!(b.earliest(), Some(Timestamp(10)));
        let due = b.take_due(Timestamp(20));
        assert_eq!(
            due,
            vec![
                FrameCallback {
                    surface: SurfaceId(1),
                    commit: CommitId(4)
                },
                FrameCallback {
                    surface: SurfaceId(2),
                    commit: CommitId(5)
                },
            ]
        );
        b.remove_surface(SurfaceId(1));
        assert_eq!(b.earliest(), None);
    }
}
