mod into_shiperator;
mod mixed;
#[cfg(feature = "parallel")]
mod parallel;
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
    pub(crate) entities: RawEntityIdAccess,
    pub(crate) is_exact_sized: bool,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl<S: Shiperator> Iterator for Shiper<S> {
    type Item = S::Out;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.start == self.end {
                if let Some(new_end) = self.entities.next_slice() {
                    self.start = 0;
                    self.end = new_end;

                    self.shiperator.next_slice();
                } else {
                    return None;
                }
            };

            let current = self.start;
            self.start += 1;

            if let Some(indices) = unsafe {
                self.shiperator
                    .captain_indices_of(&mut self.entities, current)
            } {
                return unsafe { Some(self.shiperator.get_data(indices)) };
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let max_len = self.end - self.start + self.entities.follow_up_len();

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
        loop {
            if self.start == self.end {
                if let Some(new_end) = self.entities.next_slice() {
                    self.start = 0;
                    self.end = new_end;

                    self.shiperator.next_slice();
                } else {
                    return init;
                }
            };

            while self.start < self.end {
                let current = self.start;
                self.start += 1;

                if let Some(indices) = unsafe {
                    self.shiperator
                        .captain_indices_of(&mut self.entities, current)
                } {
                    init = f(init, unsafe { self.shiperator.get_data(indices) });
                }
            }
        }
    }
}

impl<S: Shiperator> DoubleEndedIterator for Shiper<S> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            if self.start == self.end {
                if let Some(new_end) = self.entities.next_slice() {
                    self.start = 0;
                    self.end = new_end;

                    self.shiperator.next_slice();
                } else {
                    return None;
                }
            };

            self.end -= 1;

            if let Some(indices) = unsafe {
                self.shiperator
                    .captain_indices_of(&mut self.entities, self.end)
            } {
                return unsafe { Some(self.shiperator.get_data(indices)) };
            }
        }
    }

    #[inline]
    fn rfold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        loop {
            if self.start == self.end {
                if let Some(new_end) = self.entities.next_slice() {
                    self.start = 0;
                    self.end = new_end;

                    self.shiperator.next_slice();
                } else {
                    return init;
                }
            };

            while self.start < self.end {
                self.end -= 1;

                if let Some(indices) = unsafe {
                    self.shiperator
                        .captain_indices_of(&mut self.entities, self.end)
                } {
                    init = f(init, unsafe { self.shiperator.get_data(indices) });
                }
            }
        }
    }
}

impl<S: Shiperator> FusedIterator for Shiper<S> {}

impl<'tmp, T: Component> ExactSizeIterator for Shiper<FullRawWindow<'tmp, T>> {
    #[inline]
    fn len(&self) -> usize {
        self.end - self.start
    }
}

impl<'tmp, T: Component, Track> ExactSizeIterator for Shiper<FullRawWindowMut<'tmp, T, Track>>
where
    Self: Iterator,
{
    #[inline]
    fn len(&self) -> usize {
        self.end - self.start
    }
}
