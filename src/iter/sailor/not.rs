use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::ShiperatorSailor;
use crate::not::Not;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::track;
use crate::tracking::{Inserted, InsertedOrModified, Modified};

impl<'tmp, T: Component> ShiperatorSailor for Not<FullRawWindow<'tmp, T>> {
    type Index = usize;

    #[inline]
    unsafe fn get_sailor_data(&self, _index: Self::Index) -> Self::Out {}

    #[inline]
    fn indices_of(&self, eid: EntityId, _: usize) -> Option<Self::Index> {
        if self.0.index_of(eid).is_some() {
            None
        } else {
            Some(usize::MAX)
        }
    }

    #[inline]
    fn index_from_usize(index: usize) -> Self::Index {
        index
    }
}

impl<'tmp, T: Component, Track> ShiperatorSailor for Not<FullRawWindowMut<'tmp, T, Track>> {
    type Index = usize;

    #[inline]
    unsafe fn get_sailor_data(&self, _index: Self::Index) -> Self::Out {}

    #[inline]
    fn indices_of(&self, eid: EntityId, _: usize) -> Option<Self::Index> {
        if self.0.index_of(eid).is_some() {
            None
        } else {
            Some(usize::MAX)
        }
    }

    #[inline]
    fn index_from_usize(index: usize) -> Self::Index {
        index
    }
}

impl<'tmp, T: Component> ShiperatorSailor for Not<Inserted<FullRawWindow<'tmp, T>>> {
    type Index = usize;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        (self.0).0.get_sailor_data(index)
    }

    #[inline]
    fn indices_of(&self, eid: EntityId, _: usize) -> Option<Self::Index> {
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
    fn index_from_usize(index: usize) -> Self::Index {
        index
    }
}

macro_rules! impl_shiperator_sailor_not_inserted {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorSailor for Not<Inserted<FullRawWindowMut<'tmp, T, $track>>> {
                type Index = usize;

                #[inline]
                unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
                    (self.0).0.get_sailor_data(index)
                }

                #[inline]
                fn indices_of(&self, eid: EntityId, _: usize, ) -> Option<Self::Index> {
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
                fn index_from_usize(index: usize) -> Self::Index {
                    index
                }
            }
        )+
    }
}

impl_shiperator_sailor_not_inserted![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_sailor_not_inserted![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp, T: Component> ShiperatorSailor for Not<Modified<FullRawWindow<'tmp, T>>> {
    type Index = usize;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        &*(self.0).0.data.add(index)
    }

    #[inline]
    fn indices_of(&self, eid: EntityId, _: usize) -> Option<Self::Index> {
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
    fn index_from_usize(index: usize) -> Self::Index {
        index
    }
}

macro_rules! impl_shiperator_sailor_not_modified {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorSailor for Not<Modified<FullRawWindowMut<'tmp, T, $track>>> {
                type Index = usize;

                #[inline]
                unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
                    (self.0).0.get_sailor_data(index)
                }

                #[inline]
                fn indices_of(&self, eid: EntityId, _: usize, ) -> Option<Self::Index> {
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
                fn index_from_usize(index: usize) -> Self::Index {
                    index
                }
            }
        )+
    }
}

impl_shiperator_sailor_not_modified![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_sailor_not_modified![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp, T: Component> ShiperatorSailor for Not<InsertedOrModified<FullRawWindow<'tmp, T>>> {
    type Index = usize;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        &*(self.0).0.data.add(index)
    }

    #[inline]
    fn indices_of(&self, eid: EntityId, _: usize) -> Option<Self::Index> {
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
    fn index_from_usize(index: usize) -> Self::Index {
        index
    }
}

macro_rules! impl_shiperator_sailor_not_inserted_or_modified {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorSailor for Not<InsertedOrModified<FullRawWindowMut<'tmp, T, $track>>> {
                type Index = usize;

                #[inline]
                unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
                    (self.0).0.get_sailor_data(index)
                }

                #[inline]
                fn indices_of(&self, eid: EntityId, _: usize, ) -> Option<Self::Index> {
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
                fn index_from_usize(index: usize) -> Self::Index {
                    index
                }
            }
        )+
    }
}

impl_shiperator_sailor_not_inserted_or_modified![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_sailor_not_inserted_or_modified![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];
