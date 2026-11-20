mod not;
mod or;
mod tracking;

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::{EntityCursor, SegmentEntry, SegmentRow};
use crate::r#mut::Mut;
use crate::optional::Optional;
use crate::sparse_set::{FullRawWindow, FullRawWindowMut, TRACKING_CHUNK_SIZE};
use crate::track;
use crate::tracking::{ModificationTracking, Tracking};

/// Provides iteration costs, component lookup, and data access for a storage iterator.
///
/// Implementing this trait is safe on its own: iterators only drive a Shiperator returned by
/// an [`IntoShiperator`](crate::iter::IntoShiperator) implementation, and that `unsafe impl`
/// vouches for the behavior each method documents under "Implementations".\
/// The "Safety" sections list what callers guarantee.
pub trait Shiperator {
    /// The type this Shiperator will yield.
    ///
    /// This is often `&T` or `&mut T`.
    type Out;
    /// Type this Shiperator is indexed with.\
    /// This is often `usize` or a tuple of `usize` for multiple storages.
    type Index;

    /// Prefers evaluating this captain before sailors on the single-captain fast path.
    /// The planner still selects the captain; matching results stay unchanged.
    ///
    /// ### Implementations
    ///
    /// Enable for filters whose rejection is cheaper than a sailor lookup.
    /// The default preserves tuple order. Wrappers should account for their own lookup costs.
    const PREFER_CAPTAIN_FIRST: bool = false;

    /// Enables candidate ranges and candidate-weighted splitting. Defaults to full segments.
    const HAS_CANDIDATE_PLAN: bool = false;

    /// Whether a required input proves this entire query empty.
    /// Unknown inputs keep the default `false`; optional and negated inputs cannot prove it.
    #[inline]
    fn has_no_candidates(&self) -> bool {
        false
    }

    /// Whether the current captain's plan keeps the whole segment as candidates.
    /// Exact tracking and join checks still apply. Exact windows keep the whole segment.
    #[inline]
    fn has_dense_candidates(&self) -> bool {
        self.is_exact_sized()
    }

    /// Prepares modification bounds for captain candidates; `None` restores sailor marking.
    ///
    /// ### Implementations
    ///
    /// Bounds may be conservative. This must not access component data or change matching results.
    /// The default keeps per-item marking. Wrappers should forward their children's active ranges.
    ///
    /// ### Safety
    ///
    /// A range belongs to the installed segment. Until preparation changes, this node only yields
    /// captain candidates from that range. Prepare `None` before using it as a sailor.
    #[inline]
    unsafe fn prepare_tracking(&mut self, _range: Option<core::ops::Range<usize>>) {}

    /// Returns a data accessor for a prepared captain range.
    ///
    /// The default uses ordinary access. Implementations may mark conservative bounds once
    /// and return an accessor without per-item summary work.
    ///
    /// ### Safety
    ///
    /// The range belongs to the installed segment. The returned accessor is only used with
    /// indices resolved for candidates from this range, while the storage borrow stays alive.
    #[inline]
    unsafe fn prepare_bulk(
        &self,
        _range: core::ops::Range<usize>,
    ) -> unsafe fn(&Self, Self::Index) -> Self::Out {
        Self::get_data
    }

    /// Releases planning data when this node becomes a sailor, preserving its lookup results.
    #[inline]
    fn unpick(&mut self) {}

    /// Counts candidate slots, before exact matching, in the current segment's range.
    /// Counts must be additive across subranges and identical for clones.
    ///
    /// ### Safety
    /// The range belongs to the installed segment, whose plan must stay alive.
    #[inline]
    unsafe fn candidate_count(&self, start: usize, end: usize) -> usize {
        end - start
    }

    /// Splits a range after `count` candidate slots. Must return an index in `start..=end`.
    /// Both halves keep exactly the candidate counts reported by `candidate_count`.
    ///
    /// ### Safety
    /// The range belongs to the installed segment, its plan stays alive, and `count`
    /// does not exceed its candidate count.
    #[inline]
    unsafe fn candidate_split_at(&self, start: usize, _end: usize, count: usize) -> usize {
        start + count
    }

    /// Returns the next candidate range within `index..end` of the current segment.
    ///
    /// ### Implementations
    ///
    /// Return a nonempty subrange, or `end..end` when no candidates remain.
    /// Candidates before its start must fail to match. Candidates inside still need checking.
    /// The default keeps the whole range. This method must not access component data.
    ///
    /// ### Safety
    ///
    /// The range must belong to the current segment and its plan must still be alive.
    #[inline]
    unsafe fn next_candidate_range(&self, index: usize, end: usize) -> core::ops::Range<usize> {
        index..end
    }

    /// Returns the number of nodes in this Shiperator's tree, itself included.
    ///
    /// Plan rows hold one state per node, in preorder.\
    /// Composite Shiperators use it to skip the states of their sailor children.
    ///
    /// ### Implementations
    ///
    /// Must be the number of nodes [`describe`](Shiperator::describe) appends.
    fn node_count() -> usize;
    /// Appends this node's kind and current state to `row`, followed by its children's.
    ///
    /// `active` is `false` when the node only answers sailor lookups,
    /// its whole subtree is then described as sailors.
    ///
    /// ### Implementations
    ///
    /// The kinds don't depend on the state. Installing the described states with
    /// [`enter_segment`](Shiperator::enter_segment) gives back the current state.
    fn describe(&self, active: bool, row: &mut SegmentRow);
    /// Installs the state this node and its active children hold for the segment about to be iterated.
    ///
    /// Takes this node's state from `entry` then routes `entry` through the active children,
    /// skipping the states of the sailor ones. Sailors are left untouched.
    ///
    /// ### Implementations
    ///
    /// Takes exactly the states of this node's subtree from `entry`.\
    /// Only installs state: it must not create a reference to a component or call back into
    /// a consumer, another split half might hold items of these storages.
    ///
    /// ### Safety
    ///
    /// - `entry`'s next states must be the states of this node's subtree, from a row built for
    ///   this Shiperator, in a plan or alone
    /// - until the next entry, the candidates this Shiperator receives as a captain must come from
    ///   the segment this row was built for
    /// - the plan `entry` reads from, if any, must stay alive as long as this Shiperator, or a
    ///   clone of it, resolves captain indices
    unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>);
    /// Approximation of how much time iterating this Shiperator will take.\
    /// This helps pick the fastest Shiperator when iterating multiple storages.
    ///
    /// Iterating a `Vec` of length 100 will return 100.
    fn sail_time(&self) -> usize;
    /// `true` when this Shiperator cannot return `None`.
    ///
    /// ### Implementations
    ///
    /// Only returns `true` when no candidate of the current segment can be rejected.
    fn is_exact_sized(&self) -> bool;
    /// Returns the absolute index in this node's storage that `index` points to.
    ///
    /// Only plain component windows have a single storage index, other nodes return `None`.\
    /// `Mixed` uses it to reject candidates an earlier segment already walked,
    /// without creating any component reference.
    ///
    /// ### Implementations
    ///
    /// When it returns `Some`, it's the index of the component `get_data` reads for `index`.
    fn storage_index(index: &Self::Index) -> Option<usize>;
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
    /// It only resolves indices, it never creates a component reference.
    ///
    /// ### Implementations
    ///
    /// Returns the indices of the components of the candidate's entity, or `None` when it isn't
    /// a match. Clones resolve every candidate to the same indices.
    ///
    /// # Safety
    ///
    /// - `entities` must not be empty
    /// - `index` must be the local index of the current candidate of `entities`
    /// - the plan of the last segment this Shiperator entered, if any, must still be alive
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
    /// ### Implementations
    ///
    /// The result doesn't depend on the segment currently iterated.
    fn sailor_indices_of(&self, entity_id: EntityId) -> Option<Self::Index>;
    /// Returns the component at `index`.
    ///
    /// `index` is already resolved, implementations must not add their segment start again.
    ///
    /// ### Implementations
    ///
    /// Distinct entities never get overlapping mutable outputs. The output stays valid as long
    /// as the storage borrow the Shiperator was built from.
    ///
    /// # Safety
    ///
    /// `index` must be a value returned by `captain_indices_of` or `sailor_indices_of`
    /// for this Shiperator.
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out;
    /// Folds the items of the candidates left in `entities`, without entering any other segment.
    ///
    /// Every candidate left is consumed, accepted or not, `entities` is empty afterwards.\
    /// `f` receives the accumulator, the candidate's [`EntityId`] and its item.
    /// The id is only captured when `WITH_ID` is `true`, `f` receives `None` otherwise.
    ///
    /// The default implementation resolves each candidate with
    /// [`captain_indices_of`](Shiperator::captain_indices_of).
    /// Composite Shiperators override it to pick a loop suited to the segment once,
    /// outside the loop.
    ///
    /// ### Implementations
    ///
    /// Yields the items the default implementation would, in the same order, and consumes each
    /// candidate before creating its item.
    ///
    /// ### Safety
    ///
    /// - `entities`' local indices must be local indices of the segment this Shiperator
    ///   currently iterates, `entities` doesn't have to start at its beginning
    /// - the plan of the last segment this Shiperator entered, if any, must still be alive
    #[inline]
    unsafe fn fold_segment<const WITH_ID: bool, B, F>(
        &self,
        entities: &mut EntityCursor,
        init: B,
        f: F,
    ) -> B
    where
        F: FnMut(B, Option<EntityId>, Self::Out) -> B,
    {
        // SAFE: `fold_candidates` calls the resolver with a non-empty cursor and its current
        // candidate's local index, a local index of our current segment per our caller.
        // Our caller keeps the plan of our last segment alive.
        // `captain_indices_of`'s results are valid for `get_data`.
        unsafe {
            fold_candidates::<Self, WITH_ID, B, F, _>(self, entities, init, f, |entities, index| {
                self.captain_indices_of(entities, index)
            })
        }
    }
}

/// Folds the items of the candidates left in `entities`, resolving each with `resolve`.
///
/// Every bulk loop goes through here so they all consume candidates in the same order:
/// resolve the indices, which includes rejecting covered candidates, capture the id when
/// `WITH_ID` is `true`, consume the candidate, create the item and finally call `f`.
///
/// ### Safety
///
/// Calling `resolve` with a non-empty cursor and the local index of its current candidate
/// must be safe, and the indices it returns must be valid indices for `shiperator.get_data`.
/// The cursor must belong to the installed segment, whose plan must stay alive.
#[inline(always)]
pub(crate) unsafe fn fold_candidates<S, const WITH_ID: bool, B, F, R>(
    shiperator: &S,
    entities: &mut EntityCursor,
    init: B,
    f: F,
    resolve: R,
) -> B
where
    S: Shiperator + ?Sized,
    F: FnMut(B, Option<EntityId>, S::Out) -> B,
    R: Fn(&EntityCursor, usize) -> Option<S::Index>,
{
    // SAFE: The caller guarantees resolved indices are valid for this window.
    unsafe {
        fold_candidates_with::<S, WITH_ID, B, F, R, _>(
            shiperator,
            entities,
            init,
            f,
            resolve,
            |index| shiperator.get_data(index),
        )
    }
}

// Keep progress local so component writes cannot alias the hot cursor.
// Publish it on normal return and unwinding alike.
struct CursorProgress<'a> {
    target: &'a mut EntityCursor,
    cursor: EntityCursor,
}
impl Drop for CursorProgress<'_> {
    fn drop(&mut self) {
        *self.target = self.cursor;
    }
}

// Same contracts, with a data accessor valid for every resolved index.
#[inline(always)]
pub(crate) unsafe fn fold_candidates_with<S, const WITH_ID: bool, B, F, R, G>(
    shiperator: &S,
    entities: &mut EntityCursor,
    init: B,
    mut f: F,
    resolve: R,
    get_data: G,
) -> B
where
    S: Shiperator + ?Sized,
    F: FnMut(B, Option<EntityId>, S::Out) -> B,
    R: Fn(&EntityCursor, usize) -> Option<S::Index>,
    G: Fn(S::Index) -> S::Out,
{
    let mut progress = CursorProgress {
        cursor: *entities,
        target: entities,
    };
    let entities = &mut progress.cursor;
    // Dense plans need exact checks, but no candidate range lookups.
    if !S::HAS_CANDIDATE_PLAN || shiperator.is_exact_sized() || shiperator.has_dense_candidates() {
        // SAFE: The same cursor, resolver and data access contracts apply.
        return unsafe {
            fold_all_candidates::<S, WITH_ID, B, F, R, G>(
                shiperator, entities, init, f, resolve, get_data,
            )
        };
    }
    let mut acc = init;

    while !entities.is_empty() {
        let index = entities.local_index();
        // SAFE: Our caller keeps this segment and its plan valid for the remaining cursor.
        let range = unsafe { shiperator.next_candidate_range(index, index + entities.len()) };
        entities.advance_by(range.start - index);
        while entities.local_index() < range.end {
            let Some(indices) = resolve(entities, entities.local_index()) else {
                entities.advance();
                continue;
            };

            let entity_id = if WITH_ID {
                // SAFE: The cursor isn't empty, we haven't consumed its current candidate yet.
                Some(unsafe { entities.current() })
            } else {
                None
            };

            // The candidate is consumed before its item exists. If `f` panics, nothing can
            // resume from this candidate and create its item a second time.
            entities.advance();

            // SAFE: Our caller guarantees `resolve`'s indices are valid for `get_data`.
            let item = get_data(indices);

            acc = f(acc, entity_id, item);
        }
    }

    acc
}

// Same contracts as `fold_candidates`, without candidate range lookups.
#[inline(always)]
unsafe fn fold_all_candidates<S, const WITH_ID: bool, B, F, R, G>(
    _shiperator: &S,
    entities: &mut EntityCursor,
    init: B,
    mut f: F,
    resolve: R,
    get_data: G,
) -> B
where
    S: Shiperator + ?Sized,
    F: FnMut(B, Option<EntityId>, S::Out) -> B,
    R: Fn(&EntityCursor, usize) -> Option<S::Index>,
    G: Fn(S::Index) -> S::Out,
{
    let mut acc = init;
    while !entities.is_empty() {
        let Some(indices) = resolve(entities, entities.local_index()) else {
            entities.advance();
            continue;
        };

        let entity_id = if WITH_ID {
            // SAFE: The cursor isn't empty, we haven't consumed its current candidate yet.
            Some(unsafe { entities.current() })
        } else {
            None
        };

        // The candidate is consumed before its item exists. If `f` panics, nothing can
        // resume from this candidate and create its item a second time.
        entities.advance();

        // SAFE: Our caller guarantees `resolve`'s indices are valid for `get_data`.
        let item = get_data(indices);

        acc = f(acc, entity_id, item);
    }
    acc
}

impl<'tmp, T: Component, Track: Tracking> Shiperator for FullRawWindow<'tmp, T, Track> {
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
    fn storage_index(index: &Self::Index) -> Option<usize> {
        Some(*index)
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        // SAFE: Our caller passes an index we resolved. Captain indices are below the storage's
        // length per `set_segment_start`'s contract, sailor indices come from its sparse array.
        // The storage stays borrowed for 'tmp.
        unsafe { &*self.data.add(index) }
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
                fn storage_index(index: &Self::Index) -> Option<usize> {
                    Some(*index)
                }

                #[inline]
                unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                    // SAFE: Our caller passes an index we resolved, see `FullRawWindow::get_data`.
                    // The storage stays exclusively borrowed for 'tmp. Traversals consume each
                    // candidate before creating its item and reject the ones an earlier segment
                    // yielded, so no other item points to this component.
                    unsafe { &mut *self.data.add(index) }
                }
            }
        )+
    }
}

impl_shiperator_no_mut![track::Untracked track::Insertion track::InsertionAndDeletion track::InsertionAndRemoval track::InsertionAndDeletionAndRemoval track::Deletion track::DeletionAndRemoval track::Removal];

impl<'tmp, T: Component, Track: ModificationTracking> FullRawWindowMut<'tmp, T, Track> {
    // Same index contract as get_data; the candidate's summary already covers current.
    #[inline(always)]
    unsafe fn get_prepared_data(&self, index: usize) -> Mut<'tmp, T> {
        unsafe {
            Mut {
                flag: Some(&mut *self.modification_data.add(index)),
                chunk: None,
                current: self.current,
                data: &mut *self.data.add(index),
            }
        }
    }

    // Same contracts as fold_segment; summaries are conservative upper bounds.
    #[inline]
    unsafe fn fold_tracked_segment<const WITH_ID: bool, B, F>(
        &self,
        entities: &mut EntityCursor,
        mut acc: B,
        mut f: F,
    ) -> B
    where
        F: FnMut(B, Option<EntityId>, Mut<'tmp, T>) -> B,
    {
        if entities.is_empty() {
            return acc;
        }
        let start = self.resolve_index(entities.local_index());
        let end = start + entities.len();
        if !self.chunks_prepared {
            // SAFE: The storage borrow keeps the summary alive.
            unsafe { self.modification_summary.as_ref() }.prepare_range(
                start..end,
                self.dense_len,
                self.current,
            );
        }

        // Mark before yielding: items may be retained and mutated after this fold returns.
        // Per-item flags remain lazy; read-only callbacks only broaden the chunk summary.
        while !entities.is_empty() {
            let index = self.resolve_index(entities.local_index());
            let entity_id = if WITH_ID {
                // SAFE: The candidate is unconsumed and belongs to the installed segment.
                Some(unsafe { entities.current() })
            } else {
                None
            };
            entities.advance();
            // SAFE: This unique candidate was consumed before yielding; its summary covers current.
            let item = unsafe {
                Mut {
                    flag: Some(&mut *self.modification_data.add(index)),
                    chunk: None,
                    current: self.current,
                    data: &mut *self.data.add(index),
                }
            };
            acc = f(acc, entity_id, item);
        }
        acc
    }
}

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

                #[inline(always)]
                unsafe fn enter_segment(&mut self, entry: &mut SegmentEntry<'_>) {
                    // SAFE: Our caller guarantees `entry` holds our window's state for the segment about to be iterated.
                    unsafe { self.enter_window(entry) };
                }

                #[inline(always)]
                unsafe fn prepare_tracking(&mut self, range: Option<core::ops::Range<usize>>) {
                    self.chunks_prepared = range.is_some();
                    if let Some(range) = range {
                        let start = self.resolve_index(range.start);
                        let end = self.resolve_index(range.end);
                        // SAFE: The caller supplies this window's installed candidate range.
                        unsafe { self.modification_summary.as_ref() }
                            .prepare_range(start..end, self.dense_len, self.current);
                    }
                }

                #[inline(always)]
                unsafe fn prepare_bulk(
                    &self, range: core::ops::Range<usize>,
                ) -> unsafe fn(&Self, Self::Index) -> Self::Out {
                    if !self.chunks_prepared {
                        let start = self.resolve_index(range.start);
                        let end = self.resolve_index(range.end);
                        // SAFE: The caller supplies this window's installed candidate range.
                        unsafe { self.modification_summary.as_ref() }
                            .prepare_range(start..end, self.dense_len, self.current);
                    }
                    Self::get_prepared_data
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
                fn storage_index(index: &Self::Index) -> Option<usize> {
                    Some(*index)
                }

                #[inline]
                unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
                    // SAFE: Same as the untracked windows, the component is only reachable through
                    // this item. `$track` tracks modification, so the storage keeps one
                    // modification timestamp per component and `index` is one of them.
                    unsafe {
                        Mut {
                            flag: Some(&mut *self.modification_data.add(index)),
                            chunk: if self.chunks_prepared { None } else {
                                let chunks = self.modification_writers.as_ref();
                                debug_assert!(index / TRACKING_CHUNK_SIZE < chunks.len());
                                // Each tracked component has a summary for its chunk.
                                Some(chunks.get_unchecked(index / TRACKING_CHUNK_SIZE))
                            },
                            current: self.current,
                            data: &mut *self.data.add(index),
                        }
                    }
                }

                #[inline]
                unsafe fn fold_segment<const WITH_ID: bool, B, F>(
                    &self,
                    entities: &mut EntityCursor,
                    init: B,
                    f: F,
                ) -> B
                where
                    F: FnMut(B, Option<EntityId>, Self::Out) -> B,
                {
                    // SAFE: The caller supplies this window's installed segment and unique items.
                    unsafe { self.fold_tracked_segment::<WITH_ID, _, _>(entities, init, f) }
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
    fn storage_index(_index: &Self::Index) -> Option<usize> {
        None
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        // SAFE: Our caller passes an index we returned, the local index of a candidate of our only
        // segment, which walks the whole slice.
        unsafe { *self.get_unchecked(index) }
    }
}

impl<'tmp, T: Component, Track: Tracking> Shiperator for Optional<FullRawWindow<'tmp, T, Track>> {
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
    fn storage_index(_index: &Self::Index) -> Option<usize> {
        None
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        if let Some(index) = index {
            // SAFE: Our caller upholds `get_data`'s contract for the inner window.
            Some(unsafe { self.0.get_data(index) })
        } else {
            None
        }
    }
}

impl<'tmp, T: Component, Track: Tracking> Shiperator for Optional<FullRawWindowMut<'tmp, T, Track>>
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
        // SAFE: Our caller upholds `captain_indices_of`'s contract for the inner window.
        Some(unsafe { self.0.captain_indices_of(entities, index) })
    }

    #[inline]
    fn sailor_indices_of(&self, entity_id: EntityId) -> Option<Self::Index> {
        Some(self.0.sailor_indices_of(entity_id))
    }

    #[inline]
    fn storage_index(_index: &Self::Index) -> Option<usize> {
        None
    }

    #[inline]
    unsafe fn get_data(&self, index: Self::Index) -> Self::Out {
        if let Some(index) = index {
            // SAFE: Our caller upholds `get_data`'s contract for the inner window.
            Some(unsafe { self.0.get_data(index) })
        } else {
            None
        }
    }
}
