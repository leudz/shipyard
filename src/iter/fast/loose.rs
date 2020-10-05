use super::abstract_mut::FastAbstractMut;
use crate::iter::with_id::LastId;
use crate::storage::EntityId;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::Producer;

pub struct FastLoose<Storage> {
    pub(crate) storage: Storage,
    pub(crate) indices: *const EntityId,
    pub(crate) current: usize,
    pub(crate) end: usize,
    pub(crate) mask: u16,
}

unsafe impl<Storage: Send> Send for FastLoose<Storage> {}

impl<Storage: FastAbstractMut> Iterator for FastLoose<Storage> {
    type Item = <Storage as FastAbstractMut>::Out;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            self.current += 1;

            let id = unsafe { *self.indices.add(self.current - 1) };

            let data_indices = unsafe {
                self.storage
                    .indices_of_unchecked(id, self.current - 1, self.mask)
            };

            Some(unsafe { FastAbstractMut::get_datas(&self.storage, data_indices) })
        } else {
            None
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact = self.end - self.current;

        (exact, Some(exact))
    }
    #[inline]
    fn fold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        while self.current < self.end {
            self.current += 1;

            let id = unsafe { *self.indices.add(self.current - 1) };

            let data_indices = unsafe {
                self.storage
                    .indices_of_unchecked(id, self.current - 1, self.mask)
            };

            init = f(init, unsafe {
                FastAbstractMut::get_datas(&self.storage, data_indices)
            });
        }

        init
    }
}

impl<Storage: FastAbstractMut> ExactSizeIterator for FastLoose<Storage> {
    #[inline]
    fn len(&self) -> usize {
        self.end - self.current
    }
}

impl<Storage: FastAbstractMut> DoubleEndedIterator for FastLoose<Storage> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            self.end -= 1;

            let id = unsafe { *self.indices.add(self.end) };

            let data_indices =
                unsafe { self.storage.indices_of_unchecked(id, self.end, self.mask) };

            Some(unsafe { FastAbstractMut::get_datas(&self.storage, data_indices) })
        } else {
            None
        }
    }
    #[inline]
    fn rfold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        while self.current < self.end {
            self.end -= 1;

            let id = unsafe { *self.indices.add(self.end) };

            let data_indices =
                unsafe { self.storage.indices_of_unchecked(id, self.end, self.mask) };

            init = f(init, unsafe {
                FastAbstractMut::get_datas(&self.storage, data_indices)
            });
        }

        init
    }
}

impl<Storage: FastAbstractMut> LastId for FastLoose<Storage> {
    #[inline]
    unsafe fn last_id(&self) -> EntityId {
        *self.indices.add(self.current - 1)
    }
    #[inline]
    unsafe fn last_id_back(&self) -> EntityId {
        *self.indices.add(self.end + 1)
    }
}

#[cfg(feature = "parallel")]
impl<Storage: FastAbstractMut + Clone + Send> Producer for FastLoose<Storage> {
    type Item = <Self as Iterator>::Item;
    type IntoIter = Self;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self
    }
    #[inline]
    fn split_at(mut self, index: usize) -> (Self, Self) {
        let second_half = FastLoose {
            storage: self.storage.clone(),
            current: self.current + index,
            end: self.end,
            indices: self.indices,
            mask: self.mask,
        };

        self.end = second_half.current;

        (self, second_half)
    }
}
