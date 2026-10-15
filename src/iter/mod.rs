mod into_shiperator;
mod mixed;
#[cfg(feature = "parallel")]
mod parallel;
mod plan;
mod shiperator;
mod with_id;

pub use crate::iter_component::{IntoIterRef, IterComponent};
pub use crate::not::Not;
pub use crate::optional::Optional;
pub use crate::or::{OneOfTwo, Or};
#[doc(inline)]
pub use crate::sparse_set::RawEntityIdAccess;
pub use crate::tracking::{Inserted, InsertedOrModified, Modified};
pub use into_shiperator::{IntoIter, IntoShiperator};
pub use mixed::Mixed;
#[cfg(feature = "parallel")]
#[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
pub use parallel::ParShiperator;
pub use shiperator::Shiperator;
pub use with_id::WithId;

use crate::component::Component;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use core::iter::FusedIterator;

/// Handles storages iteration.
pub struct Shiper<S> {
    pub(crate) shiperator: S,
    /// Candidates left to iterate.
    pub(crate) entities: RawEntityIdAccess,
    pub(crate) is_exact_sized: bool,
}

impl<S: Shiperator> Shiper<S> {
    /// Makes sure the current range has a candidate, moving to the follow-up ranges as needed.
    ///
    /// Returns `false` once all ranges are exhausted.
    #[inline(always)]
    pub(crate) fn has_candidate(&mut self) -> bool {
        // Follow-ups can be empty, we skip them until we find a candidate.
        while self.entities.is_current_empty() {
            if !self.entities.next_slice() {
                return false;
            }

            self.shiperator.next_slice();
        }

        true
    }
}

impl<S: Shiperator> Iterator for Shiper<S> {
    type Item = S::Out;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        while self.has_candidate() {
            let local_index = self.entities.local_index();
            // SAFE: `has_candidate` returned `true` so the current range isn't exhausted
            // and `local_index` is the local index of its current candidate.
            let indices = unsafe {
                self.shiperator
                    .captain_indices_of(&self.entities, local_index)
            };

            // The candidate is consumed whether it matched or not.
            self.entities.advance();

            if let Some(indices) = indices {
                // SAFE: `indices` was just returned by `captain_indices_of`.
                return Some(unsafe { self.shiperator.get_data(indices) });
            }
        }

        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let max_len = self.entities.remaining_len();

        if self.is_exact_sized {
            (max_len, Some(max_len))
        } else {
            (0, Some(max_len))
        }
    }

    #[inline]
    fn fold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        // We start at the first unconsumed candidate, `next` might have consumed a prefix.
        while self.has_candidate() {
            let local_index = self.entities.local_index();
            // SAFE: `has_candidate` returned `true` so the current range isn't exhausted
            // and `local_index` is the local index of its current candidate.
            let indices = unsafe {
                self.shiperator
                    .captain_indices_of(&self.entities, local_index)
            };

            // We consume the candidate before creating any output or calling `f`.
            self.entities.advance();

            if let Some(indices) = indices {
                // SAFE: `indices` was just returned by `captain_indices_of`.
                init = f(init, unsafe { self.shiperator.get_data(indices) });
            }
        }

        init
    }
}

impl<S: Shiperator> FusedIterator for Shiper<S> {}

impl<'tmp, T: Component> ExactSizeIterator for Shiper<FullRawWindow<'tmp, T>> {
    #[inline]
    fn len(&self) -> usize {
        self.entities.remaining_len()
    }
}

impl<'tmp, T: Component, Track> ExactSizeIterator for Shiper<FullRawWindowMut<'tmp, T, Track>>
where
    Self: Iterator,
{
    #[inline]
    fn len(&self) -> usize {
        self.entities.remaining_len()
    }
}
