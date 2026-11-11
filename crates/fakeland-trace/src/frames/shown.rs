//! Content that reaches the screen through a commit without content. A latch can take a content
//! commit and a later commit without content of the same surface together. The scheduler then
//! discards the content commit and presents the later one, while the refresh shows the buffer of
//! the content commit. This module finds that content commit for each presentation.

use std::collections::HashMap;

use fakeland_types::{CommitId, SurfaceId};

/// The newest discarded content commit of each surface since its last presentation.
#[derive(Debug, Default)]
pub(crate) struct Unshown {
    by_surface: HashMap<SurfaceId, CommitId>,
}

impl Unshown {
    /// Notes a discarded commit.
    pub(crate) fn discarded(&mut self, surface: SurfaceId, commit: CommitId, has_content: bool) {
        if has_content {
            self.by_surface.insert(surface, commit);
        }
    }

    /// Notes a presentation of a surface. `has_content` tells whether the presented commit
    /// changes the content, `None` for an unknown commit. Returns the discarded content commit
    /// whose buffer the refresh shows instead of a new buffer of the presented commit.
    pub(crate) fn presented(
        &mut self,
        surface: SurfaceId,
        has_content: Option<bool>,
    ) -> Option<CommitId> {
        let unshown = self.by_surface.remove(&surface);
        match has_content {
            Some(false) => unshown,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: SurfaceId = SurfaceId(1);
    const T: SurfaceId = SurfaceId(2);

    #[test]
    fn a_commit_without_content_shows_the_newest_discarded_content() {
        let mut u = Unshown::default();
        u.discarded(S, CommitId(1), true);
        u.discarded(S, CommitId(2), true);
        u.discarded(S, CommitId(3), false);
        u.discarded(T, CommitId(4), true);
        assert_eq!(u.presented(S, Some(false)), Some(CommitId(2)));
        assert_eq!(u.presented(S, Some(false)), None);
        assert_eq!(u.presented(T, Some(false)), Some(CommitId(4)));
    }

    #[test]
    fn a_content_commit_shows_its_own_buffer() {
        let mut u = Unshown::default();
        u.discarded(S, CommitId(1), true);
        assert_eq!(u.presented(S, Some(true)), None);
        assert_eq!(u.presented(S, Some(false)), None);
        u.discarded(S, CommitId(2), true);
        assert_eq!(u.presented(S, None), None);
        assert_eq!(u.presented(S, Some(false)), None);
    }
}
