mod captain;
mod into_shiperator;
mod mixed;
mod output;
#[cfg(feature = "parallel")]
mod parallel;
mod sailor;
mod with_id;

#[doc(inline)]
pub use crate::sparse_set::RawEntityIdAccess;
pub use captain::ShiperatorCaptain;
pub use into_shiperator::{IntoIter, IntoShiperator};
pub use mixed::Mixed;
pub use output::ShiperatorOutput;
#[cfg(feature = "parallel")]
#[cfg_attr(docsrs, doc(cfg(feature = "thread_local")))]
pub use parallel::ParShiperator;
pub use sailor::ShiperatorSailor;
pub use with_id::WithId;

use crate::component::Component;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use core::iter::FusedIterator;

/// Handles storages iteration.
pub struct Shiperator<S> {
    pub(crate) shiperator: S,
    pub(crate) entities: RawEntityIdAccess,
    pub(crate) is_exact_sized: bool,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl<S: ShiperatorCaptain + ShiperatorSailor> Iterator for Shiperator<S> {
    type Item = S::Out;

    #[inline]
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

            if self.is_exact_sized {
                return unsafe { Some(self.shiperator.get_captain_data(current)) };
            } else {
                let entity_id = unsafe { self.entities.get(current) };

                if let Some(indices) = self.shiperator.indices_of(entity_id, current) {
                    return unsafe { Some(self.shiperator.get_sailor_data(indices)) };
                }
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

            if self.is_exact_sized {
                while self.start < self.end {
                    let current = self.start;
                    self.start += 1;

                    init = f(init, unsafe { self.shiperator.get_captain_data(current) });
                }
            } else {
                while self.start < self.end {
                    let current = self.start;
                    self.start += 1;
                    let entity_id = unsafe { self.entities.get(current) };

                    if let Some(indices) = self.shiperator.indices_of(entity_id, current) {
                        init = f(init, unsafe { self.shiperator.get_sailor_data(indices) });
                    }
                }
            }
        }
    }
}

impl<S: ShiperatorCaptain + ShiperatorSailor> DoubleEndedIterator for Shiperator<S> {
    #[inline]
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

            if self.is_exact_sized {
                return unsafe { Some(self.shiperator.get_captain_data(self.end)) };
            } else {
                let entity_id = unsafe { self.entities.get(self.end) };

                if let Some(indices) = self.shiperator.indices_of(entity_id, self.end) {
                    return unsafe { Some(self.shiperator.get_sailor_data(indices)) };
                }
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

            if self.is_exact_sized {
                while self.start < self.end {
                    self.end -= 1;

                    init = f(init, unsafe { self.shiperator.get_captain_data(self.end) });
                }
            } else {
                while self.start < self.end {
                    self.end -= 1;
                    let entity_id = unsafe { self.entities.get(self.end) };

                    if let Some(indices) = self.shiperator.indices_of(entity_id, self.end) {
                        init = f(init, unsafe { self.shiperator.get_sailor_data(indices) });
                    }
                }
            }
        }
    }
}

impl<S: ShiperatorCaptain + ShiperatorSailor> FusedIterator for Shiperator<S> {}

impl<'tmp, T: Component> ExactSizeIterator for Shiperator<FullRawWindow<'tmp, T>> {
    fn len(&self) -> usize {
        self.end - self.start
    }
}

impl<'tmp, T: Component, Track> ExactSizeIterator for Shiperator<FullRawWindowMut<'tmp, T, Track>>
where
    Self: Iterator,
{
    fn len(&self) -> usize {
        self.end - self.start
    }
}
