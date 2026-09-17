mod not;
mod or;
mod tracking;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::optional::Optional;
use crate::r#mut::Mut;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut, RawEntityIdAccess};
use crate::track;

/// Provides iteration costs, component lookup, and data access for a storage iterator.
pub trait Shiperator {
    /// The type this Shiperator will yield.
    ///
    /// This is often `&T` or `&mut T`.
    type Out;
    /// Type this Shiperator is indexed with.\
    /// This is often `usize` or a tuple of `usize` for multiple storages.
    type Index;

    /// Shiperators might iterate multiple slices of `EntityId`s.\
    /// This function is called on the switch to the next slice.
    fn next_slice(&mut self);
    /// Approximation of how much time iterating this Shiperator will take.\
    /// This helps pick the fastest Shiperator when iterating multiple storages.
    ///
    /// Iterating a `Vec` of length 100 will return 100.
    fn sail_time(&self) -> usize;
    /// `true` when this Shiperator cannot return `None`.
    fn is_exact_sized(&self) -> bool;
    /// Returns the indices of the components matching `entity_id`.
    ///
    /// `entity_id` is the entity present at `index` in the Captain Shiperator.
    ///
    /// This function is only called when this Shiperator is the Captain.
    ///
    /// # Safety
    ///
    /// `index` must be the current dense index of this Shiperator's entity list.\
    /// `entity_id` must be at `index` in the Captain Shiperator.
    unsafe fn captain_indices_of(
        &self,
        entities: &mut RawEntityIdAccess,
        index: usize,
    ) -> Option<Self::Index>;
    /// Provides access to components when the storage doesn't drive the iteration.
    ///
    /// This function is only called when this Shiperator is a Sailor.
    ///
    /// Returns the index of the component owned by `entity_id`.
    fn sailor_indices_of(&self, entity_id: EntityId) -> Option<Self::Index>;
    /// Returns the component at `index`.
    ///
    /// # Safety
    ///
    /// `index` must be a value returned by `captain_indices_of` or `sailor_indices_of`
    /// for this Shiperator.
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out;
}

impl<'tmp, T: Component> Shiperator for FullRawWindow<'tmp, T> {
    type Out = &'tmp T;
    type Index = usize;

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    fn sail_time(&self) -> usize {
        self.dense_len
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        true
    }

    #[inline]
    unsafe fn captain_indices_of(
        &self,
        _entities: &mut RawEntityIdAccess,
        index: usize,
    ) -> Option<Self::Index> {
        Some(index)
    }

    #[inline]
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
        self.index_of(eid)
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        &*self.data.add(index)
    }
}

macro_rules! impl_shiperator_no_mut {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> Shiperator for FullRawWindowMut<'tmp, T, $track> {
                type Out = &'tmp mut T;
                type Index = usize;

                #[inline]
                fn next_slice(&mut self) {}

                #[inline]
                fn sail_time(&self) -> usize {
                    self.dense_len
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    true
                }

                #[inline]
                unsafe fn captain_indices_of(&self, _entities: &mut RawEntityIdAccess, index: usize) -> Option<Self::Index> {
                    Some(index)
                }

                #[inline]
                fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
                    self.index_of(eid)
                }

                #[inline]
                unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                    &mut *self.data.add(index)
                }
            }
        )+
    }
}

impl_shiperator_no_mut![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];

macro_rules! impl_shiperator_mut {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> Shiperator for FullRawWindowMut<'tmp, T, $track> {
                type Out = Mut<'tmp, T>;
                type Index = usize;

                #[inline]
                fn next_slice(&mut self) {}

                #[inline]
                fn sail_time(&self) -> usize {
                    self.dense_len
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    true
                }

                #[inline]
                unsafe fn captain_indices_of(&self, _entities: &mut RawEntityIdAccess, index: usize) -> Option<Self::Index> {
                    Some(index)
                }

                #[inline]
                fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
                    self.index_of(eid)
                }

                #[inline]
                unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                    Mut {
                        flag: Some(&mut *self.modification_data.add(index)),
                        current: self.current,
                        data: &mut *self.data.add(index),
                    }
                }
            }
        )+
    }
}

impl_shiperator_mut![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp> Shiperator for &'tmp [EntityId] {
    type Out = EntityId;
    type Index = usize;

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    fn sail_time(&self) -> usize {
        self.len()
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        true
    }

    #[inline]
    unsafe fn captain_indices_of(
        &self,
        _entities: &mut RawEntityIdAccess,
        index: usize,
    ) -> Option<Self::Index> {
        Some(index)
    }

    #[inline]
    fn sailor_indices_of(&self, _entity_id: EntityId) -> Option<Self::Index> {
        unreachable!()
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        *self.get_unchecked(index)
    }
}

impl<'tmp, T: Component> Shiperator for Optional<FullRawWindow<'tmp, T>> {
    type Out = Option<&'tmp T>;
    type Index = Option<usize>;

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    fn sail_time(&self) -> usize {
        self.0.sail_time()
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    #[inline]
    unsafe fn captain_indices_of(
        &self,
        _entities: &mut RawEntityIdAccess,
        _index: usize,
    ) -> Option<Self::Index> {
        unreachable!()
    }

    #[inline]
    fn sailor_indices_of(&self, entity_id: EntityId) -> Option<Self::Index> {
        Some(self.0.sailor_indices_of(entity_id))
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        if let Some(index) = index {
            Some(self.0.get_data(index))
        } else {
            None
        }
    }
}

impl<'tmp, T: Component, Track> Shiperator for Optional<FullRawWindowMut<'tmp, T, Track>>
where
    FullRawWindowMut<'tmp, T, Track>: Shiperator<Index = usize>,
{
    type Out = Option<<FullRawWindowMut<'tmp, T, Track> as Shiperator>::Out>;
    type Index = Option<usize>;

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    fn sail_time(&self) -> usize {
        self.0.sail_time()
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    #[inline]
    unsafe fn captain_indices_of(
        &self,
        entities: &mut RawEntityIdAccess,
        index: usize,
    ) -> Option<Self::Index> {
        Some(self.0.captain_indices_of(entities, index))
    }

    #[inline]
    fn sailor_indices_of(&self, entity_id: EntityId) -> Option<Self::Index> {
        Some(self.0.sailor_indices_of(entity_id))
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        index.map(|index| self.0.get_data(index))
    }
}
