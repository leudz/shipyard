use super::abstract_mut::FastAbstractMut;
use super::chunk::FastChunk;
use super::chunk_exact::FastChunkExact;
use crate::entity_id::EntityId;
use crate::iter::with_id::LastId;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::Producer;

#[allow(missing_docs)]
pub struct FastTight<Storage> {
    pub(super) storage: Storage,
    pub(super) current: usize,
    pub(super) end: usize,
}

impl<Storage: FastAbstractMut> FastTight<Storage> {
    /// Transforms this iterator into a chunked iterator, yielding arrays of elements.  
    /// If the number of elements can't be perfectly divided by `step` the last array will be smaller.
    pub fn into_chunk(self, step: usize) -> FastChunk<Storage> {
        FastChunk {
            storage: self.storage,
            current: self.current,
            end: self.end,
            step,
        }
    }
    /// Transforms this iterator into a chunked iterator, yielding arrays of elements.  
    /// If the number of elements can't be perfectly divided by `step` the last elements will be ignored.
    /// Use `remainder` to retreive them.
    pub fn into_chunk_exact(self, step: usize) -> FastChunkExact<Storage> {
        FastChunkExact {
            storage: self.storage,
            current: self.current,
            end: self.end,
            step,
        }
    }
}

impl<Storage: FastAbstractMut> Iterator for FastTight<Storage> {
    type Item = <Storage as FastAbstractMut>::Out;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            self.current += 1;

            Some(unsafe { FastAbstractMut::get_data(&self.storage, self.current - 1) })
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

            init = f(init, unsafe {
                FastAbstractMut::get_data(&self.storage, self.current - 1)
            });
        }

        init
    }
}

impl<Storage: FastAbstractMut> ExactSizeIterator for FastTight<Storage> {
    #[inline]
    fn len(&self) -> usize {
        self.end - self.current
    }
}

impl<Storage: FastAbstractMut> DoubleEndedIterator for FastTight<Storage> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            self.end -= 1;

            Some(unsafe { FastAbstractMut::get_data(&self.storage, self.end) })
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

            init = f(init, unsafe {
                FastAbstractMut::get_data(&self.storage, self.end)
            });
        }

        init
    }
}

impl<Storage: FastAbstractMut> LastId for FastTight<Storage> {
    #[inline]
    unsafe fn last_id(&self) -> EntityId {
        self.storage.get_id(self.current - 1)
    }
    #[inline]
    unsafe fn last_id_back(&self) -> EntityId {
        self.storage.get_id(self.end + 1)
    }
}

#[cfg(feature = "parallel")]
impl<Storage: FastAbstractMut + Clone + Send> Producer for FastTight<Storage> {
    type Item = <Self as Iterator>::Item;
    type IntoIter = Self;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self
    }
    #[inline]
    fn split_at(mut self, index: usize) -> (Self, Self) {
        let second_half = FastTight {
            storage: self.storage.clone(),
            current: self.current + index,
            end: self.end,
        };

        self.end = second_half.current;

        (self, second_half)
    }
}
