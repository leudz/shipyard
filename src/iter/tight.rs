use super::abstract_mut::AbstractMut;
use super::with_id::LastId;
use crate::entity_id::EntityId;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::Producer;

#[allow(missing_docs)]
pub struct Tight<Storage> {
    pub(crate) storage: Storage,
    pub(crate) current: usize,
    pub(crate) end: usize,
}

impl<Storage: AbstractMut> Iterator for Tight<Storage> {
    type Item = Storage::Out;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            self.current += 1;

            Some(unsafe { self.storage.get_data(self.current - 1) })
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

            init = f(init, unsafe { self.storage.get_data(self.current - 1) });
        }

        init
    }
}

impl<Storage: AbstractMut> ExactSizeIterator for Tight<Storage> {
    #[inline]
    fn len(&self) -> usize {
        self.end - self.current
    }
}

impl<Storage: AbstractMut> DoubleEndedIterator for Tight<Storage> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            self.end -= 1;

            Some(unsafe { self.storage.get_data(self.end) })
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

            init = f(init, unsafe { self.storage.get_data(self.end) });
        }

        init
    }
}

impl<Storage: AbstractMut> LastId for Tight<Storage> {
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
impl<Storage: AbstractMut + Clone + Send> Producer for Tight<Storage> {
    type Item = <Self as Iterator>::Item;
    type IntoIter = Self;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self
    }
    #[inline]
    fn split_at(mut self, index: usize) -> (Self, Self) {
        let second_half = Tight {
            storage: self.storage.clone(),
            current: self.current + index,
            end: self.end,
        };

        self.end = second_half.current;

        (self, second_half)
    }
}
