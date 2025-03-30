mod not;
mod or;
mod tracking;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::ShiperatorOutput;
use crate::optional::Optional;
use crate::r#mut::Mut;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::track;

/// Provides access to components when the storage doesn't drives the iteration.
pub trait ShiperatorSailor: ShiperatorOutput {
    /// Type this Shiperator is indexed with.\
    /// This is often `usize` or a tuple of `usize` for multiple storages.
    type Index;

    /// Returns the component at `index`.
    ///
    /// # Safety
    ///
    /// `index` must be the value returned by `indices_of`.\
    /// It shouldn't exceed the length of the storage.
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out;
    /// Returns the index of the component with id `entity_id`.
    fn indices_of(&self, entity_id: EntityId, index: usize) -> Option<Self::Index>;
    /// When a `Mixed` iterator flags a storage as captain, it can skip `indices_of` and directly use `index`
    /// for this one Shiperator.\
    /// But at the type level there is no way to express this since the captain is picked at runtime.
    ///
    /// We also cannot use `From<usize>` since it's only implemented for tuples up to 12 items.
    fn index_from_usize(index: usize) -> Self::Index;
}

impl<'tmp, T: Component> ShiperatorSailor for FullRawWindow<'tmp, T> {
    type Index = usize;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        &*self.data.add(index)
    }

    #[inline]
    fn indices_of(&self, eid: EntityId, _: usize) -> Option<Self::Index> {
        self.index_of(eid)
    }

    #[inline]
    fn index_from_usize(index: usize) -> Self::Index {
        index
    }
}

macro_rules! impl_shiperator_sailor_no_mut {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorSailor for FullRawWindowMut<'tmp, T, $track> {
                type Index = usize;

                #[inline]
                unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
                    &mut *self.data.add(index)
                }

                #[inline]
                fn indices_of(&self, eid: EntityId, _: usize, ) -> Option<Self::Index> {
                    self.index_of(eid)
                }

                #[inline]
                fn index_from_usize(index: usize) -> Self::Index {
                    index
                }
            }
        )+
    }
}

impl_shiperator_sailor_no_mut![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];

macro_rules! impl_shiperator_sailor_mut {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorSailor for FullRawWindowMut<'tmp, T, $track> {
                type Index = usize;

                #[inline]
                unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
                    Mut {
                        flag: Some(&mut *self.modification_data.add(index)),
                        current: self.current,
                        data: &mut *self.data.add(index),
                    }
                }

                #[inline]
                fn indices_of(&self, eid: EntityId, _: usize, ) -> Option<Self::Index> {
                    self.index_of(eid)
                }

                #[inline]
                fn index_from_usize(index: usize) -> Self::Index {
                    index
                }
            }
        )+
    }
}

impl_shiperator_sailor_mut![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp> ShiperatorSailor for &'tmp [EntityId] {
    type Index = EntityId;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        index
    }

    #[inline]
    fn indices_of(&self, entity_id: EntityId, _index: usize) -> Option<Self::Index> {
        Some(entity_id)
    }

    #[inline]
    fn index_from_usize(_index: usize) -> Self::Index {
        unreachable!()
    }
}

impl<'tmp, T: Component> ShiperatorSailor for Optional<FullRawWindow<'tmp, T>> {
    type Index = Option<usize>;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        if let Some(index) = index {
            Some(self.0.get_sailor_data(index))
        } else {
            None
        }
    }

    #[inline]
    fn indices_of(&self, entity_id: EntityId, index: usize) -> Option<Self::Index> {
        Some(self.0.indices_of(entity_id, index))
    }

    #[inline]
    fn index_from_usize(_index: usize) -> Self::Index {
        unreachable!()
    }
}

impl<'tmp, T: Component, Track> ShiperatorSailor for Optional<FullRawWindowMut<'tmp, T, Track>>
where
    Optional<FullRawWindowMut<'tmp, T, Track>>:
        ShiperatorOutput<Out = Option<<FullRawWindowMut<'tmp, T, Track> as ShiperatorOutput>::Out>>,
    FullRawWindowMut<'tmp, T, Track>: ShiperatorSailor<Index = usize>,
{
    type Index = Option<usize>;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        index.map(|index| self.0.get_sailor_data(index))
    }

    #[inline]
    fn indices_of(&self, entity_id: EntityId, index: usize) -> Option<Self::Index> {
        Some(self.0.indices_of(entity_id, index))
    }

    #[inline]
    fn index_from_usize(_index: usize) -> Self::Index {
        unreachable!()
    }
}
