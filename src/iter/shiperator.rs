mod not;
mod or;
mod tracking;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::{EntityCursor, SegmentEntry, SegmentRow};
use crate::optional::Optional;
use crate::r#mut::Mut;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut};
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

    /// Returns the number of nodes in this Shiperator's tree, itself included.
    ///
    /// Plan rows hold one state per node, in preorder.\
    /// Composite Shiperators use it to skip the states of their sailor children.
    fn node_count() -> usize;
    /// Appends this node's kind and current state to `row`, followed by its children's.
    ///
    /// `active` is `false` when the node only answers sailor lookups,
    /// its whole subtree is then described as sailors.
    fn describe(&self, active: bool, row: &mut SegmentRow);
    /// Installs the state this node and its active children hold for the segment about to be iterated.
    ///
    /// Takes this node's state from `entry` then routes `entry` through the active children,
    /// skipping the states of the sailor ones. Sailors are left untouched.
    ///
    /// ### Safety
    ///
    /// - `entry`'s next states must be the states of this node's subtree, from a row of a plan
    ///   built for this Shiperator
    /// - until the next entry, the candidates this Shiperator receives as a captain must come from
    ///   the segment this row was built for
    unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>);
    /// Approximation of how much time iterating this Shiperator will take.\
    /// This helps pick the fastest Shiperator when iterating multiple storages.
    ///
    /// Iterating a `Vec` of length 100 will return 100.
    fn sail_time(&self) -> usize;
    /// `true` when this Shiperator cannot return `None`.
    fn is_exact_sized(&self) -> bool;
    /// Returns the indices of the components matching the current candidate of `entities`.
    ///
    /// `index` is the candidate's index local to the segment currently iterated.\
    /// Shiperators resolve it to the absolute index of their component by adding
    /// the start of their segment.\
    /// The candidate's [`EntityId`] is read with `EntityCursor::current`.
    ///
    /// This doesn't consume the candidate, the caller advances `entities` afterwards.
    ///
    /// The returned indices are absolute and are passed as is to [`get_data`](Shiperator::get_data).
    ///
    /// This function is only called when this Shiperator is the Captain.
    ///
    /// # Safety
    ///
    /// - `entities` must not be empty
    /// - `index` must be the local index of the current candidate of `entities`
    unsafe fn captain_indices_of(
        &self,
        entities: &EntityCursor,
        index: usize,
    ) -> Option<Self::Index>;
    /// Provides access to components when the storage doesn't drive the iteration.
    ///
    /// This function is only called when this Shiperator is a Sailor.
    ///
    /// Returns the absolute index of the component owned by `entity_id`.
    ///
    /// The result doesn't depend on the segment currently iterated.
    fn sailor_indices_of(&self, entity_id: EntityId) -> Option<Self::Index>;
    /// Returns the component at `index`.
    ///
    /// `index` is already resolved, implementations must not add their segment start again.
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
    fn node_count() -> usize {
        1
    }

    #[inline]
    fn describe(&self, active: bool, row: &mut SegmentRow) {
        self.describe_window(active, row);
    }

    #[inline]
    unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
        // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
        unsafe { self.enter_window(entry) };
    }

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
        _entities: &EntityCursor,
        index: usize,
    ) -> Option<Self::Index> {
        Some(self.resolve_index(index))
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
                fn node_count() -> usize {
                    1
                }

                #[inline]
                fn describe(&self, active: bool, row: &mut SegmentRow) {
                    self.describe_window(active, row);
                }

                #[inline]
                unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
                    // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
                    unsafe { self.enter_window(entry) };
                }

                #[inline]
                fn sail_time(&self) -> usize {
                    self.dense_len
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    true
                }

                #[inline]
                unsafe fn captain_indices_of(&self, _entities: &EntityCursor, index: usize) -> Option<Self::Index> {
                    Some(self.resolve_index(index))
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
                fn node_count() -> usize {
                    1
                }

                #[inline]
                fn describe(&self, active: bool, row: &mut SegmentRow) {
                    self.describe_window(active, row);
                }

                #[inline]
                unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
                    // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
                    unsafe { self.enter_window(entry) };
                }

                #[inline]
                fn sail_time(&self) -> usize {
                    self.dense_len
                }

                #[inline]
                fn is_exact_sized(&self) -> bool {
                    true
                }

                #[inline]
                unsafe fn captain_indices_of(&self, _entities: &EntityCursor, index: usize) -> Option<Self::Index> {
                    Some(self.resolve_index(index))
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
    fn node_count() -> usize {
        1
    }

    // An entity slice is a single segment, its local indices are already indices of the slice.
    #[inline]
    fn describe(&self, active: bool, row: &mut SegmentRow) {
        row.push_window(active, 0);
    }

    #[inline]
    unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
        let segment_start = entry.next_window();

        assert_eq!(segment_start, 0, "entity slices are a single segment");
    }

    #[inline]
    fn sail_time(&self) -> usize {
        self.len()
    }

    #[inline]
    fn is_exact_sized(&self) -> bool {
        true
    }

    // An entity slice has no segment state, `index` is always an index of the whole slice.
    #[inline]
    unsafe fn captain_indices_of(
        &self,
        _entities: &EntityCursor,
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
        _entities: &EntityCursor,
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
        entities: &EntityCursor,
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
        if let Some(index) = index {
            Some(self.0.get_data(index))
        } else {
            None
        }
    }
}
