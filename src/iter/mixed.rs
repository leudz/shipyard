use super::abstract_mut::AbstractMut;
use super::with_id::LastId;
use crate::entity_id::EntityId;
#[cfg(feature = "rayon")]
use rayon::iter::plumbing::UnindexedProducer;

#[allow(missing_docs)]
pub struct Mixed<Storage> {
    pub(super) storage: Storage,
    pub(super) indices: *const EntityId,
    pub(super) current: usize,
    pub(super) end: usize,
    pub(super) mask: u16,
    pub(super) last_id: EntityId,
}

unsafe impl<Storage: Send> Send for Mixed<Storage> {}

impl<Storage: AbstractMut> Iterator for Mixed<Storage> {
    type Item = <Storage as AbstractMut>::Out;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.end {
            self.current += 1;

            let id = unsafe { *self.indices.add(self.current - 1) };

            if let Some(data_indices) = self.storage.indices_of(id, self.current - 1, self.mask) {
                self.last_id = id;
                return Some(unsafe { self.storage.get_datas(data_indices) });
            }
        }

        None
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.end - self.current))
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

            if let Some(data_indices) = self.storage.indices_of(id, self.current - 1, self.mask) {
                self.last_id = id;
                init = f(init, unsafe { self.storage.get_datas(data_indices) });
            }
        }

        init
    }
}

impl<Storage: AbstractMut> DoubleEndedIterator for Mixed<Storage> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.current < self.end {
            self.current += 1;

            let id = unsafe { *self.indices.add(self.end - self.current - 1) };

            if let Some(data_indices) =
                self.storage
                    .indices_of(id, self.end - self.current - 1, self.mask)
            {
                self.last_id = id;
                return Some(unsafe { self.storage.get_datas(data_indices) });
            }
        }

        None
    }

    #[inline]
    fn rfold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        while self.current < self.end {
            self.current += 1;

            let id = unsafe { *self.indices.add(self.end - self.current - 1) };

            if let Some(data_indices) =
                self.storage
                    .indices_of(id, self.end - self.current - 1, self.mask)
            {
                self.last_id = id;
                init = f(init, unsafe { self.storage.get_datas(data_indices) });
            }
        }

        init
    }
}

impl<Storage: AbstractMut> LastId for Mixed<Storage> {
    #[inline]
    unsafe fn last_id(&self) -> EntityId {
        self.last_id
    }
    #[inline]
    unsafe fn last_id_back(&self) -> EntityId {
        self.last_id
    }
}

#[cfg(feature = "rayon")]
impl<Storage: AbstractMut + Clone + Send> UnindexedProducer for Mixed<Storage> {
    type Item = <Storage as AbstractMut>::Out;

    #[inline]
    fn split(mut self) -> (Self, Option<Self>) {
        let len = self.end - self.current;

        if len >= 2 {
            let clone = Mixed {
                storage: self.storage.clone(),
                indices: self.indices,
                current: self.current + (len / 2),
                end: self.end,
                mask: self.mask,
                last_id: self.last_id,
            };

            self.end = clone.current;

            (self, Some(clone))
        } else {
            (self, None)
        }
    }

    #[inline]
    fn fold_with<F>(self, folder: F) -> F
    where
        F: rayon::iter::plumbing::Folder<Self::Item>,
    {
        folder.consume_iter(self)
    }
}
