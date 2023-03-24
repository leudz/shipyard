use super::abstract_mut::AbstractMut;
use super::with_id::LastId;
use crate::entity_id::EntityId;
use alloc::vec::Vec;
use core::slice::Iter;
#[cfg(feature = "parallel")]
use rayon::iter::plumbing::UnindexedProducer;

#[allow(missing_docs)]
pub struct Mixed<Storage> {
    pub(crate) storage: Storage,
    pub(crate) indices: Iter<'static, EntityId>,
    pub(crate) count: usize,
    pub(crate) mask: u16,
    pub(crate) last_id: EntityId,
    pub(crate) rev_next_storage: Vec<Iter<'static, EntityId>>,
}

unsafe impl<Storage: Send> Send for Mixed<Storage> {}

impl<Storage: AbstractMut> Iterator for Mixed<Storage> {
    type Item = <Storage as AbstractMut>::Out;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            for &id in self.indices.by_ref() {
                self.count += 1;

                if let Some(data_indices) = self.storage.indices_of(id, self.count - 1, self.mask) {
                    self.last_id = id;
                    return Some(unsafe { self.storage.get_datas(data_indices) });
                }
            }

            let next_indices = self.rev_next_storage.pop()?;
            self.indices = next_indices;
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            0,
            Some(
                self.indices.len()
                    + self
                        .rev_next_storage
                        .iter()
                        .map(|iter| iter.len())
                        .sum::<usize>(),
            ),
        )
    }
    #[inline]
    fn fold<B, F>(mut self, mut init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        loop {
            for &id in self.indices {
                self.count += 1;

                if let Some(data_indices) = self.storage.indices_of(id, self.count - 1, self.mask) {
                    self.last_id = id;
                    init = f(init, unsafe { self.storage.get_datas(data_indices) });
                }
            }

            if let Some(next_iter) = self.rev_next_storage.pop() {
                self.indices = next_iter;
            } else {
                return init;
            }
        }
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

#[cfg(feature = "parallel")]
impl<Storage: AbstractMut + Clone + Send> UnindexedProducer for Mixed<Storage> {
    type Item = <Storage as AbstractMut>::Out;

    #[inline]
    fn split(mut self) -> (Self, Option<Self>) {
        let len = self.indices.len();

        if len >= 2 || !self.rev_next_storage.is_empty() {
            let indices = self.indices.as_slice();
            let (first, second) = indices.split_at(indices.len() / 2);
            let second_next = self
                .rev_next_storage
                .split_off(self.rev_next_storage.len() / 2);

            let clone = Mixed {
                storage: self.storage.clone(),
                indices: second.iter(),
                count: self.count + (len / 2),
                mask: self.mask,
                last_id: self.last_id,
                rev_next_storage: second_next,
            };

            self.indices = first.iter();

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
