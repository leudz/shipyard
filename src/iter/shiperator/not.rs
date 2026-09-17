use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::Shiperator;
use crate::not::Not;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut, RawEntityIdAccess};
use crate::track;
use crate::tracking::{Inserted, InsertedOrModified, Modified};

const NOT_FACTOR: f32 = 1.2;

impl<'tmp, T: Component> Shiperator for Not<FullRawWindow<'tmp, T>> {
    type Out = ();
    type Index = ();

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn sail_time(&self) -> usize {
        (self.0.sail_time() as f32 * NOT_FACTOR) as usize
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
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
        if self.0.index_of(eid).is_some() {
            None
        } else {
            Some(())
        }
    }

    #[inline]
    unsafe fn get_data(&self, _index: Self::Index) -> Self::Out {}
}

impl<'tmp, T: Component, Track> Shiperator for Not<FullRawWindowMut<'tmp, T, Track>> {
    type Out = ();
    type Index = usize;

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    fn sail_time(&self) -> usize {
        usize::MAX
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
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
        if self.0.index_of(eid).is_some() {
            None
        } else {
            Some(usize::MAX)
        }
    }

    #[inline]
    unsafe fn get_data(&self, _index: Self::Index) -> Self::Out {}
}

impl<'tmp, T: Component> Shiperator for Not<Inserted<FullRawWindow<'tmp, T>>> {
    type Out = <Inserted<FullRawWindow<'tmp, T>> as Shiperator>::Out;
    type Index = usize;

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn sail_time(&self) -> usize {
        (self.0.sail_time() as f32 * NOT_FACTOR) as usize
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    #[inline]
    unsafe fn captain_indices_of(
        &self,
        _entities: &mut RawEntityIdAccess,
        index: usize,
    ) -> Option<Self::Index> {
        if !unsafe { *(self.0).0.insertion_data.add(index) }
            .is_within((self.0).0.last_insertion, (self.0).0.current)
        {
            Some(index)
        } else {
            None
        }
    }

    #[inline]
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
        let Some(index) = (self.0).0.index_of(eid) else {
            return None;
        };

        if unsafe { *(self.0).0.insertion_data.add(index) }
            .is_within((self.0).0.last_insertion, (self.0).0.current)
        {
            None
        } else {
            Some(index)
        }
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        (self.0).0.get_data(index)
    }
}

macro_rules! impl_shiperator_not_inserted {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> Shiperator for Not<Inserted<FullRawWindowMut<'tmp, T, $track>>> {
                type Out = <Inserted<FullRawWindowMut<'tmp, T, $track>> as Shiperator>::Out;
                type Index = usize;

                #[inline]
                fn next_slice(&mut self) {}

                #[inline]
                #[allow(clippy::cast_precision_loss)]
                fn sail_time(&self) -> usize {
                    (self.0.sail_time() as f32 * NOT_FACTOR) as usize
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    false
                }

                #[inline]
                unsafe fn captain_indices_of(&self, _entities: &mut RawEntityIdAccess, index: usize) -> Option<Self::Index> {
                    if !unsafe { *(self.0).0.insertion_data.add(index) }
                        .is_within((self.0).0.last_insertion, (self.0).0.current)
                    {
                        Some(index)
                    } else {
                        None
                    }
                }

                #[inline]
                fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
                    let Some(index) = (self.0).0.index_of(eid) else {
                        return None;
                    };

                    if unsafe { *(self.0).0.insertion_data.add(index) }
                        .is_within((self.0).0.last_insertion, (self.0).0.current)
                    {
                        None
                    } else {
                        Some(index)
                    }
                }

                #[inline]
                unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                    (self.0).0.get_data(index)
                }
            }
        )+
    }
}

impl_shiperator_not_inserted![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_not_inserted![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp, T: Component> Shiperator for Not<Modified<FullRawWindow<'tmp, T>>> {
    type Out = <Modified<FullRawWindow<'tmp, T>> as Shiperator>::Out;
    type Index = usize;

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn sail_time(&self) -> usize {
        (self.0.sail_time() as f32 * NOT_FACTOR) as usize
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    unsafe fn captain_indices_of(
        &self,
        _entities: &mut RawEntityIdAccess,
        index: usize,
    ) -> Option<Self::Index> {
        if !unsafe { *(self.0).0.modification_data.add(index) }
            .is_within((self.0).0.last_modification, (self.0).0.current)
        {
            Some(index)
        } else {
            None
        }
    }

    #[inline]
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
        let Some(index) = (self.0).0.index_of(eid) else {
            return None;
        };

        if unsafe { *(self.0).0.modification_data.add(index) }
            .is_within((self.0).0.last_modification, (self.0).0.current)
        {
            None
        } else {
            Some(index)
        }
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        &*(self.0).0.data.add(index)
    }
}

macro_rules! impl_shiperator_not_modified {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> Shiperator for Not<Modified<FullRawWindowMut<'tmp, T, $track>>> {
                type Out = <Modified<FullRawWindowMut<'tmp, T, $track>> as Shiperator>::Out;
                type Index = usize;

                #[inline]
                fn next_slice(&mut self) {}

                #[inline]
                #[allow(clippy::cast_precision_loss)]
                fn sail_time(&self) -> usize {
                    (self.0.sail_time() as f32 * NOT_FACTOR) as usize
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    false
                }

                unsafe fn captain_indices_of(&self, _entities: &mut RawEntityIdAccess, index: usize) -> Option<Self::Index> {
                    if !unsafe { *(self.0).0.modification_data.add(index) }
                        .is_within((self.0).0.last_modification, (self.0).0.current)
                    {
                        Some(index)
                    } else {
                        None
                    }
                }

                #[inline]
                fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
                    let Some(index) = (self.0).0.index_of(eid) else {
                        return None;
                    };

                    if unsafe { *(self.0).0.modification_data.add(index) }
                        .is_within((self.0).0.last_modification, (self.0).0.current)
                    {
                        None
                    } else {
                        Some(index)
                    }
                }

                #[inline]
                unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                    (self.0).0.get_data(index)
                }
            }
        )+
    }
}

impl_shiperator_not_modified![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_not_modified![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp, T: Component> Shiperator for Not<InsertedOrModified<FullRawWindow<'tmp, T>>> {
    type Out = <InsertedOrModified<FullRawWindow<'tmp, T>> as Shiperator>::Out;
    type Index = usize;

    #[inline]
    fn next_slice(&mut self) {}

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn sail_time(&self) -> usize {
        (self.0.sail_time() as f32 * NOT_FACTOR) as usize
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    unsafe fn captain_indices_of(
        &self,
        _entities: &mut RawEntityIdAccess,
        index: usize,
    ) -> Option<Self::Index> {
        if !(unsafe { *(self.0).0.insertion_data.add(index) }
            .is_within((self.0).0.last_insertion, (self.0).0.current)
            || unsafe { *(self.0).0.modification_data.add(index) }
                .is_within((self.0).0.last_modification, (self.0).0.current))
        {
            Some(index)
        } else {
            None
        }
    }

    #[inline]
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
        let Some(index) = (self.0).0.index_of(eid) else {
            return None;
        };

        if unsafe { *(self.0).0.insertion_data.add(index) }
            .is_within((self.0).0.last_insertion, (self.0).0.current)
            || unsafe { *(self.0).0.modification_data.add(index) }
                .is_within((self.0).0.last_modification, (self.0).0.current)
        {
            None
        } else {
            Some(index)
        }
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        &*(self.0).0.data.add(index)
    }
}

macro_rules! impl_shiperator_not_inserted_or_modified {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> Shiperator for Not<InsertedOrModified<FullRawWindowMut<'tmp, T, $track>>> {
                type Out = <InsertedOrModified<FullRawWindowMut<'tmp, T, $track>> as Shiperator>::Out;
                type Index = usize;

                #[inline]
                fn next_slice(&mut self) {}

                #[inline]
                #[allow(clippy::cast_precision_loss)]
                fn sail_time(&self) -> usize {
                    (self.0.sail_time() as f32 * NOT_FACTOR) as usize
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    false
                }

                unsafe fn captain_indices_of(&self, _entities: &mut RawEntityIdAccess, index: usize) -> Option<Self::Index> {
                    if !(unsafe { *(self.0).0.insertion_data.add(index) }
                        .is_within((self.0).0.last_insertion, (self.0).0.current)
                        || unsafe { *(self.0).0.modification_data.add(index) }
                            .is_within((self.0).0.last_modification, (self.0).0.current))
                    {
                        Some(index)
                    } else {
                        None
                    }
                }

                #[inline]
                fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
                    let Some(index) = (self.0).0.index_of(eid) else {
                        return None;
                    };

                    if unsafe { *(self.0).0.insertion_data.add(index) }
                        .is_within((self.0).0.last_insertion, (self.0).0.current)
                        || unsafe { *(self.0).0.modification_data.add(index) }
                        .is_within((self.0).0.last_modification, (self.0).0.current)
                    {
                        None
                    } else {
                        Some(index)
                    }
                }

                #[inline]
                unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                    (self.0).0.get_data(index)
                }
            }
        )+
    }
}

impl_shiperator_not_inserted_or_modified![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_not_inserted_or_modified![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];
