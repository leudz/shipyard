use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::ShiperatorSailor;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::track;
use crate::tracking::{Inserted, InsertedOrModified, Modified};

impl<'tmp, T: Component> ShiperatorSailor for Inserted<FullRawWindow<'tmp, T>> {
    type Index = usize;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        self.0.get_sailor_data(index)
    }

    #[inline]
    fn indices_of(&self, eid: EntityId, _: usize) -> Option<Self::Index> {
        let Some(index) = self.0.index_of(eid) else {
            return None;
        };

        if unsafe { *self.0.insertion_data.add(index) }
            .is_within(self.0.last_insertion, self.0.current)
        {
            Some(index)
        } else {
            None
        }
    }

    #[inline]
    fn index_from_usize(index: usize) -> Self::Index {
        index
    }
}

macro_rules! impl_shiperator_sailor_inserted {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorSailor for Inserted<FullRawWindowMut<'tmp, T, $track>> {
                type Index = usize;

                #[inline]
                unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
                    self.0.get_sailor_data(index)
                }

                #[inline]
                fn indices_of(&self, eid: EntityId, _: usize, ) -> Option<Self::Index> {
                    let Some(index) = self.0.index_of(eid) else {
                        return None;
                    };

                    if unsafe { *self.0.insertion_data.add(index) }.is_within(self.0.last_insertion, self.0.current)
                    {
                        Some(index)
                    } else {
                        None
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

impl_shiperator_sailor_inserted![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_sailor_inserted![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp, T: Component> ShiperatorSailor for Modified<FullRawWindow<'tmp, T>> {
    type Index = usize;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        self.0.get_sailor_data(index)
    }

    #[inline]
    fn indices_of(&self, eid: EntityId, _: usize) -> Option<Self::Index> {
        let Some(index) = self.0.index_of(eid) else {
            return None;
        };

        if unsafe { *self.0.modification_data.add(index) }
            .is_within(self.0.last_modification, self.0.current)
        {
            Some(index)
        } else {
            None
        }
    }

    #[inline]
    fn index_from_usize(index: usize) -> Self::Index {
        index
    }
}

macro_rules! impl_shiperator_sailor_modified {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorSailor for Modified<FullRawWindowMut<'tmp, T, $track>> {
                type Index = usize;

                #[inline]
                unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
                    self.0.get_sailor_data(index)
                }

                #[inline]
                fn indices_of(&self, eid: EntityId, _: usize, ) -> Option<Self::Index> {
                    let Some(index) = self.0.index_of(eid) else {
                        return None;
                    };

                    if unsafe { *self.0.modification_data.add(index) }.is_within(self.0.last_modification, self.0.current)
                    {
                        Some(index)
                    } else {
                        None
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

impl_shiperator_sailor_modified![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_sailor_modified![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp, T: Component> ShiperatorSailor for InsertedOrModified<FullRawWindow<'tmp, T>> {
    type Index = usize;

    #[inline]
    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
        self.0.get_sailor_data(index)
    }

    #[inline]
    fn indices_of(&self, eid: EntityId, _: usize) -> Option<Self::Index> {
        let Some(index) = self.0.index_of(eid) else {
            return None;
        };

        if unsafe { *self.0.insertion_data.add(index) }
            .is_within(self.0.last_insertion, self.0.current)
            || unsafe { *self.0.modification_data.add(index) }
                .is_within(self.0.last_modification, self.0.current)
        {
            Some(index)
        } else {
            None
        }
    }

    #[inline]
    fn index_from_usize(index: usize) -> Self::Index {
        index
    }
}

macro_rules! impl_shiperator_sailor_inserted_or_modified {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> ShiperatorSailor for InsertedOrModified<FullRawWindowMut<'tmp, T, $track>> {
                type Index = usize;

                #[inline]
                unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
                    self.0.get_sailor_data(index)
                }

                #[inline]
                fn indices_of(&self, eid: EntityId, _: usize, ) -> Option<Self::Index> {
                    let Some(index) = self.0.index_of(eid) else {
                        return None;
                    };

                    if unsafe { *self.0.insertion_data.add(index) }.is_within(self.0.last_insertion, self.0.current)
                        || unsafe { *self.0.modification_data.add(index) }
                            .is_within(self.0.last_modification, self.0.current)
                    {
                        Some(index)
                    } else {
                        None
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

impl_shiperator_sailor_inserted_or_modified![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_sailor_inserted_or_modified![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];
