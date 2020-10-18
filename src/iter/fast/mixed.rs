use super::abstract_mut::FastAbstractMut;
use crate::iter::with_id::LastId;
use crate::sparse_set::{SparseArray, BUCKET_SIZE, SHARED_BUCKET_SIZE};
use crate::storage::EntityId;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::UnindexedProducer;

pub struct FastMixed<Storage> {
    pub(super) storage: Storage,
    pub(super) indices: *const EntityId,
    pub(super) sparse: *const SparseArray<[EntityId; BUCKET_SIZE]>,
    pub(super) shared: *const SparseArray<[EntityId; SHARED_BUCKET_SIZE]>,
    pub(super) current: usize,
    pub(super) end: usize,
    pub(super) mask: u16,
    pub(super) last_id: EntityId,
}

unsafe impl<Storage: Send> Send for FastMixed<Storage> {}

impl<Storage: FastAbstractMut> Iterator for FastMixed<Storage> {
    type Item = <Storage as FastAbstractMut>::Out;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        while self.current < self.end {
            self.current += 1;

            let id = unsafe { *self.indices.add(self.current - 1) };

            if let Some(data_indices) = self.storage.indices_of(id, self.current - 1, self.mask) {
                self.last_id = id;
                return Some(unsafe { FastAbstractMut::get_datas(&self.storage, data_indices) });
            }
        }

        if let Some(shared) = unsafe { self.shared.as_ref() } {
            while self.current - self.end % SHARED_BUCKET_SIZE < shared.len() * SHARED_BUCKET_SIZE {
                self.current += 1;

                if shared.is_valid(self.current - 1 - self.end % SHARED_BUCKET_SIZE) {
                    let id = unsafe {
                        (&*self.sparse)
                            .get_at_unchecked(self.current - 1 - self.end % SHARED_BUCKET_SIZE)
                    };

                    if let Some(data_indices) = self.storage.indices_of(id, 0, 0) {
                        self.last_id = id;
                        return Some(unsafe {
                            FastAbstractMut::get_datas(&self.storage, data_indices)
                        });
                    }
                }
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
                init = f(init, unsafe {
                    FastAbstractMut::get_datas(&self.storage, data_indices)
                });
            }
        }

        if let Some(shared) = unsafe { self.shared.as_ref() } {
            while self.current - self.end % SHARED_BUCKET_SIZE < shared.len() {
                self.current += 1;

                if shared.is_valid(self.current - 1 - self.end % SHARED_BUCKET_SIZE) {
                    let id = unsafe {
                        (&*self.sparse)
                            .get_at_unchecked(self.current - 1 - self.end % SHARED_BUCKET_SIZE)
                    };

                    if let Some(data_indices) = self.storage.indices_of(id, 0, 0) {
                        self.last_id = id;
                        init = f(init, unsafe {
                            FastAbstractMut::get_datas(&self.storage, data_indices)
                        });
                    }
                }
            }
        }

        init
    }
}

impl<Storage: FastAbstractMut> DoubleEndedIterator for FastMixed<Storage> {
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
                return Some(unsafe { FastAbstractMut::get_datas(&self.storage, data_indices) });
            }
        }

        if let Some(shared) = unsafe { self.shared.as_ref() } {
            while self.current - self.end % SHARED_BUCKET_SIZE < shared.len() * SHARED_BUCKET_SIZE {
                self.current += 1;

                if shared.is_valid(self.current - 1 - self.end % SHARED_BUCKET_SIZE) {
                    let id = unsafe {
                        (&*self.sparse)
                            .get_at_unchecked(self.current - 1 - self.end % SHARED_BUCKET_SIZE)
                    };

                    if let Some(data_indices) = self.storage.indices_of(id, 0, 0) {
                        self.last_id = id;
                        return Some(unsafe {
                            FastAbstractMut::get_datas(&self.storage, data_indices)
                        });
                    }
                }
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
                init = f(init, unsafe {
                    FastAbstractMut::get_datas(&self.storage, data_indices)
                });
            }
        }

        if let Some(shared) = unsafe { self.shared.as_ref() } {
            while self.current - self.end % SHARED_BUCKET_SIZE < shared.len() {
                self.current += 1;

                if shared.is_valid(self.current - 1 - self.end % SHARED_BUCKET_SIZE) {
                    let id = unsafe {
                        (&*self.sparse)
                            .get_at_unchecked(self.current - 1 - self.end % SHARED_BUCKET_SIZE)
                    };

                    if let Some(data_indices) = self.storage.indices_of(id, 0, 0) {
                        self.last_id = id;
                        init = f(init, unsafe {
                            FastAbstractMut::get_datas(&self.storage, data_indices)
                        });
                    }
                }
            }
        }

        init
    }
}

impl<Storage: FastAbstractMut> LastId for FastMixed<Storage> {
    #[inline]
    unsafe fn last_id(&self) -> EntityId {
        self.last_id
    }
    #[inline]
    unsafe fn last_id_back(&self) -> EntityId {
        self.last_id
    }
}

#[cfg(feature = "parallel")]
impl<Storage: FastAbstractMut + Clone + Send> UnindexedProducer for FastMixed<Storage> {
    type Item = <Storage as FastAbstractMut>::Out;

    #[inline]
    fn split(mut self) -> (Self, Option<Self>) {
        let len = self.end - self.current;

        if len >= 2 {
            let clone = FastMixed {
                storage: self.storage.clone(),
                indices: self.indices,
                sparse: self.sparse,
                shared: self.shared,
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
