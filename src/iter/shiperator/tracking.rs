use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::Shiperator;
use crate::iter::{EntityCursor, SegmentEntry, SegmentRow};
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
use crate::track;
use crate::tracking::{Inserted, InsertedOrModified, Modified};

const TRACKING_FACTOR: f32 = 2.0;

impl<'tmp, T: Component> Shiperator for Inserted<FullRawWindow<'tmp, T>> {
    type Out = <FullRawWindow<'tmp, T> as Shiperator>::Out;
    type Index = usize;

    #[inline]
    fn node_count() -> usize {
        1
    }

    #[inline]
    fn describe(&self, active: bool, row: &mut SegmentRow) {
        self.0.describe_window(active, row);
    }

    #[inline]
    unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
        // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
        unsafe { self.0.enter_window(entry) };
    }

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn sail_time(&self) -> usize {
        (self.0.sail_time() as f32 * TRACKING_FACTOR) as usize
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    #[inline]
    unsafe fn captain_indices_of(
        &self,
        _entities: &EntityCursor,
        index: usize,
    ) -> Option<Self::Index> {
        // Timestamps are stored at the component's absolute index
        let index = self.0.resolve_index(index);

        if unsafe { *self.0.insertion_data.add(index) }
            .is_within(self.0.last_insertion, self.0.current)
        {
            Some(index)
        } else {
            None
        }
    }

    #[inline]
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
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
    fn storage_index(_index: &Self::Index) -> Option<usize> {
        None
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        self.0.get_data(index)
    }
}

macro_rules! impl_shiperator_inserted {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> Shiperator for Inserted<FullRawWindowMut<'tmp, T, $track>> {
                type Out = <FullRawWindowMut<'tmp, T, $track> as Shiperator>::Out;
                type Index = usize;

                #[inline]
                fn node_count() -> usize {
                    1
                }

                #[inline]
                fn describe(&self, active: bool, row: &mut SegmentRow) {
                    self.0.describe_window(active, row);
                }

                #[inline]
                unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
                    // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
                    unsafe { self.0.enter_window(entry) };
                }

                #[inline]
                #[allow(clippy::cast_precision_loss)]
                fn sail_time(&self) -> usize {
                    (self.0.sail_time() as f32 * TRACKING_FACTOR) as usize
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    false
                }

                #[inline]
                unsafe fn captain_indices_of(&self, _entities: &EntityCursor, index: usize,) -> Option<Self::Index> {
                    // Timestamps are stored at the component's absolute index
                    let index = self.0.resolve_index(index);

                    if unsafe { *self.0.insertion_data.add(index) }
                        .is_within(self.0.last_insertion, self.0.current)
                    {
                        Some(index)
                    } else {
                        None
                    }
                }

                #[inline]
                fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
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
                fn storage_index(_index: &Self::Index) -> Option<usize> {
                    None
                }

                #[inline]
                unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                    self.0.get_data(index)
                }
            }
        )+
    }
}

impl_shiperator_inserted![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_inserted![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp, T: Component> Shiperator for Modified<FullRawWindow<'tmp, T>> {
    type Out = <FullRawWindow<'tmp, T> as Shiperator>::Out;
    type Index = usize;

    #[inline]
    fn node_count() -> usize {
        1
    }

    #[inline]
    fn describe(&self, active: bool, row: &mut SegmentRow) {
        self.0.describe_window(active, row);
    }

    #[inline]
    unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
        // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
        unsafe { self.0.enter_window(entry) };
    }

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn sail_time(&self) -> usize {
        (self.0.sail_time() as f32 * TRACKING_FACTOR) as usize
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    #[inline]
    unsafe fn captain_indices_of(
        &self,
        _entities: &EntityCursor,
        index: usize,
    ) -> Option<Self::Index> {
        // Timestamps are stored at the component's absolute index
        let index = self.0.resolve_index(index);

        if unsafe { *self.0.modification_data.add(index) }
            .is_within(self.0.last_modification, self.0.current)
        {
            Some(index)
        } else {
            None
        }
    }

    #[inline]
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
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
    fn storage_index(_index: &Self::Index) -> Option<usize> {
        None
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        self.0.get_data(index)
    }
}

macro_rules! impl_shiperator_modified {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> Shiperator for Modified<FullRawWindowMut<'tmp, T, $track>> {
                type Out = <FullRawWindowMut<'tmp, T, $track> as Shiperator>::Out;
                type Index = usize;

                #[inline]
                fn node_count() -> usize {
                    1
                }

                #[inline]
                fn describe(&self, active: bool, row: &mut SegmentRow) {
                    self.0.describe_window(active, row);
                }

                #[inline]
                unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
                    // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
                    unsafe { self.0.enter_window(entry) };
                }

                #[inline]
                #[allow(clippy::cast_precision_loss)]
                fn sail_time(&self) -> usize {
                    (self.0.sail_time() as f32 * TRACKING_FACTOR) as usize
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    false
                }

                #[inline]
                unsafe fn captain_indices_of(&self, _entities: &EntityCursor, index: usize,) -> Option<Self::Index> {
                    // Timestamps are stored at the component's absolute index
                    let index = self.0.resolve_index(index);

                    if unsafe { *self.0.modification_data.add(index) }
                    .is_within(self.0.last_modification, self.0.current)
                    {
                        Some(index)
                    } else {
                        None
                    }
                }

                #[inline]
                fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
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
                fn storage_index(_index: &Self::Index) -> Option<usize> {
                    None
                }

                #[inline]
                unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                    self.0.get_data(index)
                }
            }
        )+
    }
}

impl_shiperator_modified![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_modified![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];

impl<'tmp, T: Component> Shiperator for InsertedOrModified<FullRawWindow<'tmp, T>> {
    type Out = <FullRawWindow<'tmp, T> as Shiperator>::Out;
    type Index = usize;

    #[inline]
    fn node_count() -> usize {
        1
    }

    #[inline]
    fn describe(&self, active: bool, row: &mut SegmentRow) {
        self.0.describe_window(active, row);
    }

    #[inline]
    unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
        // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
        unsafe { self.0.enter_window(entry) };
    }

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    fn sail_time(&self) -> usize {
        (self.0.sail_time() as f32 * TRACKING_FACTOR) as usize
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        false
    }

    #[inline]
    unsafe fn captain_indices_of(
        &self,
        _entities: &EntityCursor,
        index: usize,
    ) -> Option<Self::Index> {
        // Timestamps are stored at the component's absolute index
        let index = self.0.resolve_index(index);

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
    fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
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
    fn storage_index(_index: &Self::Index) -> Option<usize> {
        None
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        self.0.get_data(index)
    }
}

macro_rules! impl_shiperator_inserted_or_modified {
    ($($track: path)+) => {
        $(
            impl<'tmp, T: Component> Shiperator for InsertedOrModified<FullRawWindowMut<'tmp, T, $track>> {
                type Out = <FullRawWindowMut<'tmp, T, $track> as Shiperator>::Out;
                type Index = usize;

                #[inline]
                fn node_count() -> usize {
                    1
                }

                #[inline]
                fn describe(&self, active: bool, row: &mut SegmentRow) {
                    self.0.describe_window(active, row);
                }

                #[inline]
                unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
                    // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
                    unsafe { self.0.enter_window(entry) };
                }

                #[inline]
                #[allow(clippy::cast_precision_loss)]
                fn sail_time(&self) -> usize {
                    (self.0.sail_time() as f32 * TRACKING_FACTOR) as usize
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    false
                }

                #[inline]
                unsafe fn captain_indices_of(&self, _entities: &EntityCursor, index: usize,) -> Option<Self::Index> {
                    // Timestamps are stored at the component's absolute index
                    let index = self.0.resolve_index(index);

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
                fn sailor_indices_of(&self, eid: EntityId) -> Option<Self::Index> {
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
                fn storage_index(_index: &Self::Index) -> Option<usize> {
                    None
                }

                #[inline]
                unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                    self.0.get_data(index)
                }
            }
        )+
    }
}

impl_shiperator_inserted_or_modified![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];
impl_shiperator_inserted_or_modified![track::Modification track::InsertionAndModification track::InsertionAndModificationAndDeletion track::InsertionAndModificationAndRemoval track::ModificationAndDeletion track::ModificationAndRemoval track::ModificationAndDeletionAndRemoval track::All];
