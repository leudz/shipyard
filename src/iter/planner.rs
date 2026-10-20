//! Construction-only choice between an ordinary walk and walks over group ranges.
//!
//! While a query is built, each node reports what it knows about grouping in a
//! [`ConstructionMetadata`], next to its entities, never inside them.
//! A window's points to its storage's group metadata, nothing is copied: the storage stays
//! borrowed while the query is built, and the metadata is dropped before the iterator is returned.
//!
//! Picking a walk reads that metadata in two passes:
//! - Estimation, bottom-up as constructors run. A `Mixed` hands its immediate children to
//!   [`choose`], which picks its ordinary captain. Children without groups stop there.
//!   Otherwise [`select`] also estimates its grouped [`Alternative`]s: walks over the ranges
//!   containing each set of overlapping groups its window children fully hold (see
//!   [`cover_sets`]), and the alternatives its nested `Mixed` children reported, driven through
//!   them. Everything is kept in a [`Selection`], an alternative only keeps its estimate and where
//!   its group is, not its walks.
//! - Materialization, once, at the query's root. Constructors always install their ordinary walk.
//!   [`install_winner`] swaps in the root's cheapest alternative when it beats the ordinary walk:
//!   [`regions::grouped_entities`] lists that alternative's walks again and turns them into
//!   segments. A single segment is installed as is, several are published as a plan.
//!
//! Both passes list walks the same way, see [`GroupedCandidate::walks`].
//! Nothing here reads entities, estimates only use group metadata.
pub(crate) mod regions;
mod scratch;
#[cfg(test)]
mod tests;

use crate::component::Component;
use crate::iter::mixed::NON_CAPTAIN_FACTOR;
use crate::iter::{EntityCursor, RawEntityIdAccess, Shiperator};
use crate::sparse_set::{Groups, Ring, SparseSet};
use crate::storage::StorageId;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Reverse;
use core::fmt;
use core::ops::Range;
use core::ptr::NonNull;
use scratch::ScratchVec;

/// What a query node knows about grouping, reported by
/// [`IntoShiperator::into_shiperator`](crate::iter::IntoShiperator::into_shiperator).
///
/// Only shipyard reads it: a node's parent while building its own selection, then the query's
/// root to pick its walk. The root drops it before building the iterator, iterators and
/// [`IntoIterRef`](crate::iter::IntoIterRef) never keep it.\
/// It points to the group metadata of the storages the node borrows, it's only read while
/// they're borrowed.
#[doc(hidden)]
pub struct ConstructionMetadata(pub(crate) NodeMetadata);

impl ConstructionMetadata {
    /// Returns the metadata of a node grouping can neither use nor be prevented by.
    #[inline]
    pub(crate) fn ungrouped() -> ConstructionMetadata {
        ConstructionMetadata(NodeMetadata::Ungrouped)
    }

    /// Returns the metadata of a filter, or of a query containing one.
    #[inline]
    pub(crate) fn filtered() -> ConstructionMetadata {
        ConstructionMetadata(NodeMetadata::Filtered)
    }

    /// Returns the metadata of a plain window over `sparse_set`.
    #[inline]
    pub(crate) fn window<T: Component>(sparse_set: &SparseSet<T>) -> ConstructionMetadata {
        let metadata = NodeMetadata::of_storage(&sparse_set.groups, sparse_set.dense.len());

        ConstructionMetadata(metadata)
    }
}

/// What a query node knows about grouping, see [`ConstructionMetadata`].
///
/// Every node reports one, so variants stay small: a window's group metadata stays in its
/// storage and whatever only grouped `Mixed`s need is boxed.\
/// `W` is how a window's group metadata is reached: a [`WindowHandle`] while it travels from
/// node to node, a borrowed [`WindowMetadata`] once its parent reads it, see [`Child::new`].
#[derive(Clone)]
pub(crate) enum NodeMetadata<W = WindowHandle> {
    /// Nothing that could help or prevent grouping.
    ///
    /// A window over a storage without group range or an entity slice.
    Ungrouped,
    /// A plain window over a storage holding group ranges.
    Window(W),
    /// An `Optional`, `Not`, tracking filter or `Or`, or a query containing one.
    ///
    /// These queries keep ordinary selection all the way up to the root.
    Filtered,
    /// A `Mixed` without filter or grouped alternative, its parent can only look it up or drive
    /// through its ordinary walk.
    Mixed {
        /// See [`Selection::lookups`].
        lookups: usize,
        /// See [`OrdinaryCandidate::work`].
        ordinary_work: Option<u64>,
    },
    /// A `Mixed` without filter, with its ordinary walk and its grouped alternatives.
    ///
    /// Its parent can drive through any of these alternatives, only the query's root
    /// installs one, see [`install_winner`].
    GroupedMixed(Box<Selection>),
}

impl NodeMetadata {
    /// Returns the metadata of a plain window over a storage of `len` components with `groups`.
    ///
    /// The returned metadata points to `groups`, see [`WindowHandle::get`].
    #[inline]
    fn of_storage(groups: &Groups, len: usize) -> NodeMetadata {
        // Storages without group only pay this check, inlined in their constructor.
        if !groups.has_ranges() {
            return NodeMetadata::Ungrouped;
        }

        NodeMetadata::of_grouped_storage(groups, len)
    }

    /// Returns the metadata of a plain window over a storage of `len` components with `groups`,
    /// which holds at least one group range.
    ///
    /// See [`NodeMetadata::of_storage`].
    fn of_grouped_storage(groups: &Groups, len: usize) -> NodeMetadata {
        if groups.storage_id.is_none() {
            return NodeMetadata::Ungrouped;
        }

        // Walks are placed and estimated from these bounds without further checks,
        // so a layout outside the storage stays ordinary instead of being trusted.
        if !is_valid(groups, len) {
            return NodeMetadata::Ungrouped;
        }

        NodeMetadata::Window(WindowHandle {
            groups: NonNull::from(groups),
            len,
        })
    }
}

/// Returns `true` when the ranges of `groups` tile the start of a storage of `len` components,
/// each one trusting at most its length, followed by the ungrouped and unevaluated regions
/// inside the storage.
fn is_valid(groups: &Groups, len: usize) -> bool {
    let mut grouped_end = 0usize;
    for range in &groups.ranges {
        if range.start != grouped_end || range.trust > range.len {
            return false;
        }

        let Some(end) = grouped_end.checked_add(range.len) else {
            return false;
        };
        grouped_end = end;
    }

    grouped_end <= groups.unevaluated_start && groups.unevaluated_start <= len
}

/// Where a plain window's storage keeps its group metadata.
///
/// Nothing ties a [`ConstructionMetadata`] to the borrow of the storage it was read from,
/// so it holds a pointer. [`Child::new`] turns it back into a reference while the storage is
/// still borrowed.
#[derive(Clone, Copy)]
pub(crate) struct WindowHandle {
    /// The storage's group metadata, its layout checked when the handle was taken.
    groups: NonNull<Groups>,
    /// Number of components in the storage.
    len: usize,
}

impl WindowHandle {
    /// Returns the group metadata this handle points to, with `entities`, the window's.
    ///
    /// ### Safety
    ///
    /// The storage this handle was taken from must stay borrowed for `'m`, with its group
    /// metadata unmodified and no exclusive reference to it created.
    pub(crate) unsafe fn get<'m>(self, entities: EntityCursor) -> WindowMetadata<'m> {
        WindowMetadata {
            // SAFE: Our caller keeps the storage borrowed for 'm, the pointer we took from its
            // `Groups` stays valid and nothing modifies them meanwhile.
            groups: unsafe { self.groups.as_ref() },
            len: self.len,
            entities,
        }
    }
}

/// Group metadata of a plain window's storage, borrowed while the query is built.
///
/// ```text
/// [ Group range(s) ][ Ungrouped ][ Unevaluated ]
/// ```
#[derive(Clone, Copy)]
pub(crate) struct WindowMetadata<'m> {
    /// The storage's group metadata, its layout checked when the handle was taken.
    pub(crate) groups: &'m Groups,
    /// Number of components in the storage.
    pub(crate) len: usize,
    /// Every entity of the storage, grouped walks take their segments from it.
    pub(crate) entities: EntityCursor,
}

impl WindowMetadata<'_> {
    /// Returns the id of the storage, as its own group metadata records it.
    ///
    /// Thread-local storages have ids distinct from their `Send + Sync` counterparts.
    pub(crate) fn storage_id(&self) -> StorageId {
        self.groups
            .storage_id
            .expect("handles are only taken for storages recording their id")
    }

    /// Returns this metadata without its lifetime, see [`KeptWindow`].
    fn keep(&self) -> KeptWindow {
        KeptWindow {
            handle: WindowHandle {
                groups: NonNull::from(self.groups),
                len: self.len,
            },
            entities: self.entities,
        }
    }
}

/// A window child's [`WindowMetadata`] without its lifetime.
///
/// A [`Selection`] keeps one for each of its window children, the root reads them again to
/// place its winner.
#[derive(Clone, Copy)]
struct KeptWindow {
    handle: WindowHandle,
    entities: EntityCursor,
}

/// The [`WindowMetadata`] of each child of a `Mixed`, by position, `None` for other children.
type Windows<'m> = ScratchVec<Option<WindowMetadata<'m>>, 4>;

/// What the selector needs to know about an immediate child of a `Mixed`.
///
/// A window child borrows its storage's group metadata for `'m`.
pub(crate) struct Child<'m> {
    pub(crate) can_captain: bool,
    pub(crate) can_sailor: bool,
    /// The child's `Shiperator::sail_time`, it ranks ordinary captains.
    pub(crate) sail_time: usize,
    /// Number of candidates the child walks when it drives, `None` when it overflows.
    pub(crate) candidates: Option<usize>,
    pub(crate) metadata: NodeMetadata<WindowMetadata<'m>>,
}

impl<'m> Child<'m> {
    /// Describes a child from its capabilities, sail time, `entities` and `metadata`.
    ///
    /// `entities` and `metadata` must come from the same `into_shiperator` call.
    ///
    /// ### Safety
    ///
    /// The storages that `into_shiperator` call borrowed must stay borrowed for `'m`, with
    /// their group metadata unmodified and no exclusive reference to it created.
    #[inline]
    pub(crate) unsafe fn new(
        can_captain: bool,
        can_sailor: bool,
        sail_time: usize,
        entities: &RawEntityIdAccess,
        metadata: ConstructionMetadata,
    ) -> Child<'m> {
        let metadata = match metadata.0 {
            NodeMetadata::Ungrouped => NodeMetadata::Ungrouped,
            // SAFE: A window's handle was taken from the storage it borrows, our caller keeps
            // that storage borrowed and its group metadata unmodified for 'm.
            // A window walks every entity of its storage.
            NodeMetadata::Window(handle) => {
                NodeMetadata::Window(unsafe { handle.get(entities.cursor) })
            }
            NodeMetadata::Filtered => NodeMetadata::Filtered,
            NodeMetadata::Mixed {
                lookups,
                ordinary_work,
            } => NodeMetadata::Mixed {
                lookups,
                ordinary_work,
            },
            NodeMetadata::GroupedMixed(selection) => NodeMetadata::GroupedMixed(selection),
        };

        Child {
            can_captain,
            can_sailor,
            sail_time,
            candidates: entities.candidate_count(),
            metadata,
        }
    }

    /// Returns the group metadata of a plain window child over a grouped storage.
    fn window(&self) -> Option<&WindowMetadata<'m>> {
        match &self.metadata {
            NodeMetadata::Window(window) => Some(window),
            _ => None,
        }
    }

    /// Returns the number of components a sailor lookup of this child reads.
    ///
    /// A nested `Mixed` looks up each of its own children. Any other child is a single lookup:
    /// filters can read more, but they disable grouping so their estimate is never compared.
    #[inline]
    fn lookups(&self) -> usize {
        match &self.metadata {
            NodeMetadata::Mixed { lookups, .. } => *lookups,
            NodeMetadata::GroupedMixed(selection) => selection.lookups,
            _ => 1,
        }
    }

    /// Returns the estimated work of this child's ordinary walk, without its parent's lookups.
    ///
    /// Returns `None` when it doesn't fit in a `u64`.
    #[inline]
    fn ordinary_work(&self) -> Option<u64> {
        match &self.metadata {
            NodeMetadata::Mixed { ordinary_work, .. } => *ordinary_work,
            NodeMetadata::GroupedMixed(selection) => selection.ordinary.work,
            _ => walk_work(self.candidates?, 0),
        }
    }

    /// Moves out the selection this child reported, only a nested `Mixed` with grouped
    /// alternatives has one.
    ///
    /// The child still reports its lookups and its ordinary walk.
    fn take_selection(&mut self) -> Option<Box<Selection>> {
        let NodeMetadata::GroupedMixed(selection) = &self.metadata else {
            return None;
        };
        let ordinary = NodeMetadata::Mixed {
            lookups: selection.lookups,
            ordinary_work: selection.ordinary.work,
        };

        match core::mem::replace(&mut self.metadata, ordinary) {
            NodeMetadata::GroupedMixed(selection) => Some(selection),
            _ => unreachable!("we just matched it"),
        }
    }
}

/// Every walk a `Mixed` can drive with: its ordinary walk and its grouped alternatives.
#[derive(Clone)]
pub(crate) struct Selection {
    pub(crate) ordinary: OrdinaryCandidate,
    /// Grouped alternatives, ordered by the first child they involve.
    ///
    /// Empty when no group is usable or the query contains a filter.
    pub(crate) alternatives: Vec<Alternative>,
    /// Number of components a sailor lookup of this `Mixed` reads, one per window below it.
    pub(crate) lookups: usize,
    /// The query contains a filter, grouping stays disabled up to the root.
    filtered: bool,
    /// The metadata of each window child, by position, `None` for other children.
    ///
    /// A local alternative's walks are read from it again when it wins,
    /// see [`Selection::local_candidate`].
    windows: ScratchVec<Option<KeptWindow>, 4>,
    /// The selection of each nested `Mixed` child that has grouped alternatives,
    /// with its position. [`Route::Nested`] leads to them.
    nested: Vec<(usize, Box<Selection>)>,
}

/// The ordinary walk: a single captain walks all its candidates, the other children are looked up.
#[derive(Clone, Debug)]
pub(crate) struct OrdinaryCandidate {
    /// Position of the captain among the `Mixed`'s children.
    pub(crate) captain: usize,
    /// Estimated work, see [`walk_work`]. `None` when it doesn't fit in a `u64`.
    pub(crate) work: Option<u64>,
}

/// A grouped way for a `Mixed` to walk every one of its matches.
///
/// Only its estimate and where its group is are kept, its walks are listed again from the
/// metadata when it wins.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Alternative {
    /// Number of candidates walked, an upper bound of the matches since lookups can reject.
    pub(crate) candidates: usize,
    /// Estimated work for the `Mixed` reporting it, the lookups of all its children included.
    /// See [`walk_work`].
    pub(crate) work: u64,
    pub(crate) route: Route,
}

/// Where an [`Alternative`]'s group is, relative to the `Mixed` reporting it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Route {
    /// Walks over the cover set of the `Mixed`'s immediate window children requiring the
    /// members `required` flags, see [`GroupedCandidate`].
    Local { required: u32 },
    /// The nested `Mixed` at position `child` drives through its alternative at position
    /// `alternative`, the other children are looked up.
    ///
    /// The nested `Mixed` keeps its own captain mask and coverage, nothing is flattened.
    Nested { child: usize, alternative: usize },
}

/// Walks covering every match of a query through the ranges of one of its cover sets.
///
/// A cover set is made of the members a grouped alternative has to cover, and the ranges it
/// can walk. It's built from the group metadata of the `Mixed`'s immediate window children
/// alone, see [`cover_sets`].\
/// This is the region recipe: [`walks`](GroupedCandidate::walks) lists them in the order
/// they're emitted, the estimate and the plan both follow it.
#[derive(Clone, Debug)]
pub(crate) struct GroupedCandidate {
    /// Children holding the required members, bit `i` for child `i`.
    ///
    /// They're the union of overlapping groups the children fully hold. Only they establish
    /// completeness: every match has a component in each of them.
    pub(crate) required: u32,
    /// Every range whose group contains all required members, in the first required child's
    /// range order. Walks refer to them by position.
    pub(crate) ranges: ScratchVec<CoverRange, 2>,
    /// Every child taking part in a walk, in child order.
    pub(crate) members: ScratchVec<Member, 4>,
}

/// A child taking part in a grouped alternative, a required member or not.
#[derive(Clone, Copy)]
pub(crate) struct Member {
    /// Position of the child among the `Mixed`'s children.
    pub(crate) child: usize,
    /// Number of components in the child's storage.
    pub(crate) len: usize,
    /// Physical index where the storage's unevaluated region starts.
    pub(crate) unevaluated_start: usize,
    /// Every entity of the child's storage.
    pub(crate) entities: EntityCursor,
}

// `ScratchVec` fills its unused slots with this, it never takes part in a walk.
impl Default for Member {
    fn default() -> Member {
        Member {
            child: 0,
            len: 0,
            unevaluated_start: 0,
            entities: EntityCursor::empty(),
        }
    }
}

impl fmt::Debug for Member {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Member")
            .field("child", &self.child)
            .field("len", &self.len)
            .field("unevaluated_start", &self.unevaluated_start)
            .finish_non_exhaustive()
    }
}

/// One walk of a grouped alternative.
///
/// Range positions are logical, each member's ring places them in its own storage.
/// Turning them into physical indices is left to whoever emits the walks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Walk {
    /// Logical positions `start..start + len` of the range at position `range`,
    /// where every required member holds the same entities.
    ///
    /// The children `captains` flags read their component directly: the required members and
    /// the other members trusting these positions. Other children are looked up.
    Aligned {
        range: usize,
        start: usize,
        len: usize,
        captains: u32,
    },
    /// Logical positions `start..start + len` of the range at position `range`, past its
    /// aligned prefix, driven by the child at position `owner`.
    ///
    /// The children `captains` flags read their component directly: `owner` and, where
    /// `owner` trusts these positions, the other members trusting them too.
    /// Other children are looked up.
    Damaged {
        range: usize,
        owner: usize,
        start: usize,
        len: usize,
        captains: u32,
    },
    /// The child at position `owner` drives the physical indices `start..start + len`
    /// of its unevaluated region. All other children are looked up.
    Unevaluated {
        owner: usize,
        start: usize,
        len: usize,
    },
}

impl Walk {
    /// Returns the number of candidates of the walk.
    pub(crate) fn len(&self) -> usize {
        match *self {
            Walk::Aligned { len, .. }
            | Walk::Damaged { len, .. }
            | Walk::Unevaluated { len, .. } => len,
        }
    }

    /// Returns the children reading their component directly, bit `i` for child `i`.
    pub(crate) fn captains(&self) -> u32 {
        match *self {
            Walk::Aligned { captains, .. } | Walk::Damaged { captains, .. } => captains,
            Walk::Unevaluated { owner, .. } => 1 << owner,
        }
    }
}

/// Returns the position of each child `mask` flags, bit `i` for child `i`, in order.
pub(crate) fn mask_children(mask: u32) -> impl Iterator<Item = usize> {
    (0..u32::BITS as usize).filter(move |&child| mask & (1 << child) != 0)
}

/// Work of walking one candidate, in half candidate visits.
const CANDIDATE_WORK: u64 = 2;
/// Work of looking up one component for a candidate, `NON_CAPTAIN_FACTOR` of a candidate visit.
#[allow(clippy::cast_precision_loss, reason = "2 is exact in an f32.")]
const SAILOR_WORK: u64 = (NON_CAPTAIN_FACTOR * CANDIDATE_WORK as f32) as u64;

#[allow(
    clippy::cast_precision_loss,
    reason = "Both values are small integers."
)]
const _: () = assert!(
    SAILOR_WORK as f32 == NON_CAPTAIN_FACTOR * CANDIDATE_WORK as f32,
    "`CANDIDATE_WORK` has to express `NON_CAPTAIN_FACTOR` exactly"
);

/// Returns the estimated work of walking `candidates` candidates, looking up `lookups`
/// components for each of them.
///
/// The unit is half a candidate visit:
/// - walking a candidate costs `CANDIDATE_WORK` (2), captains read their component directly
/// - looking up a component costs `SAILOR_WORK` (1), the `NON_CAPTAIN_FACTOR` (0.5) `Mixed`
///   weighs its sailors with
///
/// So a walk costs `candidates * (2 + lookups)`.\
/// A sailor window is one lookup, a sailor `Mixed` one per window below it.
/// Every candidate is charged all its lookups, even though a rejection stops them early.
/// Coverage checks are free next to a sparse lookup.
///
/// Returns `None` when the result doesn't fit in a `u64`.
#[inline]
fn walk_work(candidates: usize, lookups: usize) -> Option<u64> {
    let per_candidate = CANDIDATE_WORK.checked_add(SAILOR_WORK.checked_mul(lookups as u64)?)?;

    u64::try_from(candidates).ok()?.checked_mul(per_candidate)
}

/// Returns the estimated work of looking up `lookups` components for each of `candidates`
/// candidates someone else walks, see [`walk_work`].
///
/// Returns `None` when the result doesn't fit in a `u64`.
#[inline]
fn lookup_work(candidates: usize, lookups: usize) -> Option<u64> {
    let per_candidate = SAILOR_WORK.checked_mul(lookups as u64)?;

    u64::try_from(candidates).ok()?.checked_mul(per_candidate)
}

/// Picks the ordinary captain of a `Mixed` with these immediate `children` and returns it,
/// along with the metadata the `Mixed` reports to its parent.
///
/// This is [`select`] for constructors, they only keep these two.\
/// `children` must have at least one child able to captain and at most one unable to sail,
/// constructors reject other queries.
#[inline(always)]
pub(crate) fn choose<const N: usize>(
    children: &mut [Child<'_>; N],
) -> (usize, ConstructionMetadata) {
    // Grouped analysis and its scratch lists stay out of ordinary constructors.
    if needs_analysis(children) {
        return choose_grouped(children);
    }

    // Without alternatives, the `Mixed` only reports its ordinary walk, as `select` would.
    let lookups = children.iter().map(Child::lookups).sum();
    let ordinary = ordinary_candidate(children, lookups);
    let metadata = if is_filtered(children) {
        NodeMetadata::Filtered
    } else {
        NodeMetadata::Mixed {
            lookups,
            ordinary_work: ordinary.work,
        }
    };

    (ordinary.captain, ConstructionMetadata(metadata))
}

/// Returns what [`choose`] does, for `children` with groups to analyze.
#[inline(never)]
fn choose_grouped<const N: usize>(children: &mut [Child<'_>; N]) -> (usize, ConstructionMetadata) {
    let selection = select(children);

    (selection.ordinary.captain, selection.into_metadata())
}

/// Returns `true` when grouped alternatives could replace the ordinary walk of a `Mixed`
/// with these immediate `children`.
#[inline]
fn needs_analysis(children: &[Child<'_>]) -> bool {
    // Only windows over grouped storages and nested `Mixed`s with alternatives bring groups.
    let has_groups = children.iter().any(|child| {
        matches!(
            child.metadata,
            NodeMetadata::Window(_) | NodeMetadata::GroupedMixed(_)
        )
    });
    // A child that can't sail has to drive, no group can replace it.
    let has_forced_captain = children.iter().any(|child| !child.can_sailor);

    has_groups && !has_forced_captain && !is_filtered(children)
}

/// Returns `true` when one of `children` is a filter or contains one.
#[inline]
fn is_filtered(children: &[Child<'_>]) -> bool {
    children
        .iter()
        .any(|child| matches!(child.metadata, NodeMetadata::Filtered))
}

/// Ranks the alternatives of a `Mixed` with these immediate `children`.
///
/// `children` must have at least one child able to captain and at most one unable to sail,
/// constructors reject other queries.\
/// The selections nested `Mixed` children reported move into the returned selection.
pub(crate) fn select<const N: usize>(children: &mut [Child<'_>; N]) -> Selection {
    let lookups = children.iter().map(Child::lookups).sum();
    let ordinary = ordinary_candidate(children, lookups);

    if !needs_analysis(children) {
        return Selection {
            ordinary,
            alternatives: Vec::new(),
            lookups,
            filtered: is_filtered(children),
            windows: ScratchVec::new(),
            nested: Vec::new(),
        };
    }

    grouped_selection(children, ordinary, lookups)
}

/// Returns the selection of a `Mixed` with these immediate `children`, at least one of them
/// bringing groups, ranking its grouped alternatives against its `ordinary` walk.
///
/// See [`select`].
fn grouped_selection<const N: usize>(
    children: &mut [Child<'_>; N],
    ordinary: OrdinaryCandidate,
    lookups: usize,
) -> Selection {
    let windows = children.each_ref().map(|child| child.window().copied());
    let nested: Vec<(usize, Box<Selection>)> = children
        .iter_mut()
        .enumerate()
        .filter_map(|(position, child)| Some((position, child.take_selection()?)))
        .collect();
    let alternatives = alternatives(&windows, &nested, lookups);
    // Our parent only reads our estimates, there's no winner to place.
    if alternatives.is_empty() {
        return Selection {
            ordinary,
            alternatives,
            lookups,
            filtered: false,
            windows: ScratchVec::new(),
            nested: Vec::new(),
        };
    }

    Selection {
        ordinary,
        alternatives,
        lookups,
        filtered: false,
        windows: windows
            .iter()
            .map(|window| window.as_ref().map(WindowMetadata::keep))
            .collect(),
        nested,
    }
}

impl Selection {
    /// Returns the cheapest grouped alternative, the first one on ties.
    pub(crate) fn cheapest(&self) -> Option<&Alternative> {
        self.alternatives
            .iter()
            .min_by_key(|alternative| alternative.work)
    }

    /// Returns the cheapest grouped alternative when it's cheaper than the ordinary walk.
    ///
    /// The ordinary walk wins ties and estimates that don't fit in a `u64`.
    pub(crate) fn winner(&self) -> Option<&Alternative> {
        let cheapest = self.cheapest()?;
        let ordinary_work = self.ordinary.work?;

        (cheapest.work < ordinary_work).then_some(cheapest)
    }

    /// Returns the selection the nested `Mixed` at position `child` reported.
    pub(crate) fn nested(&self, child: usize) -> &Selection {
        self.nested
            .iter()
            .find(|(position, _)| *position == child)
            .map(|(_, selection)| &**selection)
            .expect("nested routes lead to a child with alternatives")
    }

    /// Returns the walks of our local alternative over the cover set requiring the members
    /// `required` flags.
    ///
    /// They're listed again from our window children's metadata, as [`select`] estimated them.
    ///
    /// ### Safety
    ///
    /// The storages our window children were built from must still be borrowed, with their
    /// group metadata unmodified and no exclusive reference to it created.
    pub(crate) unsafe fn local_candidate(&self, required: u32) -> GroupedCandidate {
        let windows: Windows<'_> = self
            .windows
            .iter()
            .map(|window| {
                // SAFE: Each handle was taken from the storage a window child borrows,
                // our caller keeps it borrowed and unmodified while we read it.
                window.map(|kept| unsafe { kept.handle.get(kept.entities) })
            })
            .collect();

        GroupedCandidate::new(&windows, required)
    }

    /// Follows `alternative`'s route down to the `Mixed` owning its group.
    ///
    /// Returns that `Mixed`'s selection and the members its cover set requires.
    #[cfg(test)]
    pub(crate) fn group_owner(&self, alternative: &Alternative) -> (&Selection, u32) {
        match alternative.route {
            Route::Local { required } => (self, required),
            Route::Nested { child, alternative } => {
                let nested = self.nested(child);

                nested.group_owner(&nested.alternatives[alternative])
            }
        }
    }

    /// Returns the walks `alternative`, one of ours, ends up driving, through any nested `Mixed`.
    ///
    /// ### Safety
    ///
    /// See [`Selection::local_candidate`].
    #[cfg(test)]
    pub(crate) unsafe fn candidate(&self, alternative: &Alternative) -> GroupedCandidate {
        let (owner, required) = self.group_owner(alternative);

        // SAFE: Our caller keeps the storages of every window below us borrowed and unmodified.
        unsafe { owner.local_candidate(required) }
    }

    /// Returns the metadata of the `Mixed` this selection is about, for its parent.
    pub(crate) fn into_metadata(self) -> ConstructionMetadata {
        ConstructionMetadata(self.into_node())
    }

    /// Returns the metadata of the `Mixed` this selection is about, never a window's.
    fn into_node<W>(self) -> NodeMetadata<W> {
        if self.filtered {
            NodeMetadata::Filtered
        } else if self.alternatives.is_empty() {
            // The parent only needs our ordinary estimate, it doesn't have to be boxed.
            NodeMetadata::Mixed {
                lookups: self.lookups,
                ordinary_work: self.ordinary.work,
            }
        } else {
            NodeMetadata::GroupedMixed(Box::new(self))
        }
    }
}

/// Returns the entities driving a whole query, installing its cheapest alternative on `root`
/// when it beats its ordinary walk.
///
/// `root`, `entities` and `metadata` are what the query's constructor returned, `root` still
/// in the state it built.\
/// The metadata is consumed here, the losing alternatives are dropped with it.
///
/// Only the root may call this. Each nested `Mixed` keeps the ordinary walk its constructor
/// installed until then: its parent estimated its own alternatives with that walk and may
/// drive through it.
///
/// ### Safety
///
/// `root`, `entities` and `metadata` must come from the same `into_shiperator` call.
#[inline]
pub(crate) unsafe fn install_winner<S: Shiperator>(
    root: &mut S,
    entities: RawEntityIdAccess,
    metadata: ConstructionMetadata,
) -> RawEntityIdAccess {
    // Only a `Mixed` reports alternatives. Wrappers either forward their child's report
    // untouched or mark it filtered, so these are about `root`'s own immediate children.
    // Queries without alternatives stop here, the rest stays out of their constructor.
    let NodeMetadata::GroupedMixed(selection) = metadata.0 else {
        return entities;
    };

    // SAFE: Our caller got all three from the same `into_shiperator` call.
    unsafe { install_grouped_winner(root, entities, &selection) }
}

/// Returns the entities driving a whole query, installing the cheapest of `selection`'s
/// alternatives on `root` when it beats its ordinary walk.
///
/// See [`install_winner`].
///
/// ### Safety
///
/// `root`, `entities` and `selection` must come from the same `into_shiperator` call.
#[inline(never)]
unsafe fn install_grouped_winner<S: Shiperator>(
    root: &mut S,
    entities: RawEntityIdAccess,
    selection: &Selection,
) -> RawEntityIdAccess {
    let Some(winner) = selection.winner() else {
        return entities;
    };

    // `root` gets the first grouped segment's state, overwriting the ordinary one.
    // The ordinary entities, and any plan they embed, are dropped with `entities`.
    // SAFE: Our caller got `root` and `selection` from the same `into_shiperator` call.
    // `IntoShiperator`'s contract keeps the storages that call borrowed, with their group
    // metadata unmodified, as long as `root` lives, and we hold it.
    unsafe { regions::grouped_entities(root, selection, winner) }
}

/// Returns the ordinary walk of `children`, which read `lookups` components as sailors.
#[inline]
fn ordinary_candidate<const N: usize>(
    children: &[Child<'_>; N],
    lookups: usize,
) -> OrdinaryCandidate {
    let captain = ordinary_captain(children);
    let child = &children[captain];

    // The captain walks its own candidates, its own sailors included,
    // and we look up our other children for each of them.
    let work = child.candidates.and_then(|candidates| {
        let sailor_work = lookup_work(candidates, lookups - child.lookups())?;

        child.ordinary_work()?.checked_add(sailor_work)
    });

    OrdinaryCandidate { captain, work }
}

/// Returns the position of the ordinary captain among `children`.
#[inline]
fn ordinary_captain<const N: usize>(children: &[Child<'_>; N]) -> usize {
    // A child that can't sail has to drive.
    if let Some(captain) = children.iter().position(|child| !child.can_sailor) {
        return captain;
    }

    // Otherwise the child with the smallest sail time drives, the first one on ties.
    children
        .iter()
        .enumerate()
        .filter(|(_, child)| child.can_captain)
        .min_by_key(|(_, child)| child.sail_time)
        .map(|(position, _)| position)
        .expect("constructors reject queries without captain")
}

/// Returns every grouped alternative of a `Mixed` whose children read `lookups` components
/// as sailors.
///
/// `windows` holds the metadata of its window children and `nested` the selection of its
/// nested `Mixed` children with alternatives, see [`Selection`].\
/// Alternatives are ordered by the first child they involve: a cover set by its first
/// required member, a nested `Mixed`'s alternatives by its position.\
/// Alternatives whose estimate doesn't fit in a `u64` can't beat anything and are left out.
fn alternatives(
    windows: &[Option<WindowMetadata<'_>>],
    nested: &[(usize, Box<Selection>)],
    lookups: usize,
) -> Vec<Alternative> {
    let mut local = cover_sets(windows).peekable();
    let mut nested = nested.iter().peekable();
    let mut alternatives = Vec::new();

    for position in 0..windows.len() {
        // Only a window can start a cover set and only a nested `Mixed` has alternatives,
        // at most one of these two adds something.
        // Cover sets come ordered by their first required member.
        while let Some(required) =
            local.next_if(|required| required.trailing_zeros() as usize == position)
        {
            // Each candidate is only kept until it's estimated.
            let candidate = GroupedCandidate::new(windows, required);
            alternatives.extend(Alternative::local(&candidate, lookups));
        }

        // Groups don't cross `Mixed` boundaries: a nested `Mixed` is never a member of our
        // groups and we drive through its alternatives as a whole, its masks stay its own.
        if let Some((_, selection)) = nested.next_if(|(child, _)| *child == position) {
            let sailor_lookups = lookups - selection.lookups;
            for (index, alternative) in selection.alternatives.iter().enumerate() {
                alternatives.extend(Alternative::nested(
                    position,
                    index,
                    alternative,
                    sailor_lookups,
                ));
            }
        }
    }

    alternatives
}

/// A range whose group contains all required members of a [`GroupedCandidate`].
#[derive(Clone, Default, Debug)]
pub(crate) struct CoverRange {
    /// Canonical identity retained for topology assertions in tests.
    #[cfg(test)]
    pub(crate) group: Arc<[StorageId]>,
    /// Each child holding a member of the group, in child order, with its part of the range.
    ///
    /// The required members are always there. Other members the query holds can come along,
    /// members the query doesn't borrow never do.
    pub(crate) members: RangeMembers,
}

/// The members of a [`CoverRange`], groups usually have a handful of them.
pub(crate) type RangeMembers = ScratchVec<RangeMember, 4>;

/// A child's part of a [`CoverRange`].
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct RangeMember {
    /// Position of the child among the `Mixed`'s children.
    pub(crate) child: usize,
    pub(crate) ring: Ring,
    /// Length of the child's logical prefix of the range unchanged since the last regroup.
    pub(crate) trust: usize,
}

/// Returns the cover sets of a `Mixed`'s children, one per set of overlapping groups they fully
/// hold, ordered by their first child. Each set is given by its required members, see
/// [`GroupedCandidate::required`].
///
/// `windows` holds the metadata of each window child, see [`Selection`].\
/// A group is fully held when each of its members is held by exactly one window child.
/// With an `(A, B)` query and only an ABC group, matches without C sit outside the group's
/// ranges and only an ordinary walk finds them.\
/// Groups sharing a member are joined: with AB and AC, an entity holding A, B and C is in
/// an ABC union, not in AB or AC. So an `(A, B, C)` query covers ABC, whether or not the
/// union exists yet, and doesn't walk AB or AC.\
/// Disjoint groups give distinct sets, they compete as distinct alternatives.
pub(crate) fn cover_sets(windows: &[Option<WindowMetadata<'_>>]) -> impl Iterator<Item = u32> {
    // `joined[child]` is the set `child` is part of so far, 0 for none.
    // Our masks cap queries at 32 children, so every child has a slot.
    let mut joined = [0u32; u32::BITS as usize];

    for (position, window) in windows.iter().enumerate() {
        let Some(window) = window else {
            continue;
        };

        for range in &window.groups.ranges {
            // Every member lists the group, we only take it from its first member.
            // A held group's first member is the window we're reading.
            let Some(held) = member_mask(windows, &range.group) else {
                continue;
            };
            if held.trailing_zeros() as usize != position {
                continue;
            }

            // The group joins the sets it shares a member with. Each member of a set holds
            // the whole set, so their union is the merged set.
            let set = mask_children(held).fold(held, |set, member| set | joined[member]);
            for member in mask_children(set) {
                joined[member] = set;
            }
        }
    }

    // We read each set at its first child, so they come ordered by it.
    joined
        .into_iter()
        .take(windows.len())
        .enumerate()
        .filter(|&(position, set)| set.trailing_zeros() as usize == position)
        .map(|(_, required)| required)
}

/// Returns the children holding `group`'s members, bit `i` for child `i`,
/// `None` unless each member is held by exactly one window child.
fn member_mask(windows: &[Option<WindowMetadata<'_>>], group: &[StorageId]) -> Option<u32> {
    group.iter().try_fold(0, |mask, &storage_id| {
        let child = window_child(windows, storage_id)?;

        Some(mask | 1u32.checked_shl(u32::try_from(child).ok()?)?)
    })
}

/// Returns the position of the window child over the storage with id `storage_id`,
/// `None` when no child or several children hold it.
///
/// A storage held by several children, like A in `(&a, &a, &b)`, is ambiguous and we don't
/// pick which child would drive.
fn window_child(windows: &[Option<WindowMetadata<'_>>], storage_id: StorageId) -> Option<usize> {
    let mut holders = windows
        .iter()
        .enumerate()
        .filter(|(_, window)| {
            window
                .as_ref()
                .is_some_and(|window| window.storage_id() == storage_id)
        })
        .map(|(position, _)| position);

    let child = holders.next()?;
    holders.next().is_none().then_some(child)
}

/// Returns the metadata of the window child at position `child`.
fn window_of<'w, 'm>(
    windows: &'w [Option<WindowMetadata<'m>>],
    child: usize,
) -> &'w WindowMetadata<'m> {
    windows[child]
        .as_ref()
        .expect("group members are window children")
}

/// Returns every range whose group contains all members `required` flags.
fn containing_ranges(
    windows: &[Option<WindowMetadata<'_>>],
    required: u32,
) -> ScratchVec<CoverRange, 2> {
    // A group containing every required member is attached to each of them,
    // the first one's ranges list all of them.
    let first_window = window_of(windows, required.trailing_zeros() as usize);

    first_window
        .groups
        .ranges
        .iter()
        .filter(|range| {
            mask_children(required).all(|child| {
                range
                    .group
                    .binary_search(&window_of(windows, child).storage_id())
                    .is_ok()
            })
        })
        .map(|range| CoverRange {
            #[cfg(test)]
            group: range.group.clone(),
            members: range_members(windows, &range.group),
        })
        .collect()
}

/// Returns each child holding a member of `group` alone, in child order, with its part of `group`'s range.
fn range_members(windows: &[Option<WindowMetadata<'_>>], group: &Arc<[StorageId]>) -> RangeMembers {
    let mut members = RangeMembers::new();
    for &storage_id in group.iter() {
        let Some(child) = window_child(windows, storage_id) else {
            continue;
        };
        let range = window_of(windows, child)
            .groups
            .ranges
            .iter()
            .find(|range| Arc::ptr_eq(&range.group, group))
            .expect("every member holds a range for its group");

        members.push(RangeMember {
            child,
            ring: range.ring(),
            trust: range.trust,
        });
    }

    // Group ids are sorted by storage, we want the members in child order.
    members.sort_unstable_by_key(|member| member.child);

    members
}

impl Alternative {
    /// Returns the alternative walking `candidate`'s walks, for a `Mixed` whose children
    /// read `lookups` components as sailors.
    ///
    /// Returns `None` when the estimate doesn't fit in a `u64`.
    fn local(candidate: &GroupedCandidate, lookups: usize) -> Option<Alternative> {
        let mut candidates = 0usize;
        let mut work = 0u64;

        for walk in candidate.walks() {
            // Captains are member windows, each one would otherwise be a single lookup.
            let sailor_lookups = lookups - walk.captains().count_ones() as usize;

            candidates = candidates.checked_add(walk.len())?;
            work = work.checked_add(walk_work(walk.len(), sailor_lookups)?)?;
        }

        Some(Alternative {
            candidates,
            work,
            route: Route::Local {
                required: candidate.required,
            },
        })
    }

    /// Returns the alternative where the child at position `child` drives through `alternative`,
    /// its alternative at position `index`, and the other children read `sailor_lookups`
    /// components for each of its candidates.
    ///
    /// Returns `None` when the estimate doesn't fit in a `u64`.
    fn nested(
        child: usize,
        index: usize,
        alternative: &Alternative,
        sailor_lookups: usize,
    ) -> Option<Alternative> {
        // The child's candidates are an upper bound of its matches, we look ourselves up
        // for each of them.
        let work = alternative
            .work
            .checked_add(lookup_work(alternative.candidates, sailor_lookups)?)?;

        Some(Alternative {
            candidates: alternative.candidates,
            work,
            route: Route::Nested {
                child,
                alternative: index,
            },
        })
    }
}

impl GroupedCandidate {
    /// Returns the walks covering the cover set requiring the members `required` flags, for a
    /// `Mixed` whose window children have the metadata in `windows`.
    fn new(windows: &[Option<WindowMetadata<'_>>], required: u32) -> GroupedCandidate {
        let ranges = containing_ranges(windows, required);

        // Required members walk their unevaluated regions, any member of a range can drive part of it.
        let member_mask = ranges
            .iter()
            .flat_map(|range| &range.members)
            .fold(required, |mask, member| mask | 1 << member.child);
        let members = mask_children(member_mask)
            .map(|child| {
                let window = window_of(windows, child);

                Member {
                    child,
                    len: window.len,
                    unevaluated_start: window.groups.unevaluated_start,
                    entities: window.entities,
                }
            })
            .collect();

        GroupedCandidate {
            required,
            ranges,
            members,
        }
    }

    /// Returns the walks covering every match, in emission order. Empty walks are left out.
    ///
    /// Each call lists them again from the candidate's metadata, nothing is stored.
    pub(crate) fn walks(&self) -> impl Iterator<Item = Walk> {
        let range_walks = self
            .ranges
            .iter()
            .enumerate()
            .flat_map(|(range_index, range)| range_walks(self.required, range_index, range));

        range_walks.chain(unevaluated_walks(self.required, &self.members))
    }
}

/// Returns the walks over `range`, the range at position `range_index`, covering the members
/// `required` flags, in emission order.
fn range_walks(
    required: u32,
    range_index: usize,
    range: &CoverRange,
) -> impl Iterator<Item = Walk> {
    let required_parts = move || {
        range
            .members
            .iter()
            .filter(move |member| required & (1 << member.child) != 0)
    };
    let trusts = move || range.members.iter().map(|member| member.trust);

    // Every required member holds the same entities over the logical prefix all of them trust.
    let aligned_len = required_parts()
        .map(|member| member.trust)
        .min()
        .expect("cover sets require at least two members");

    // Members trusting the same positions hold the same entities there, so the other
    // members join the required ones up to their own trust. A member trusting less
    // doesn't shorten the prefix, it's looked up past its trust.
    let aligned = pieces(0..aligned_len, trusts(), move |position| {
        trusting(range, position)
    })
    .map(move |(positions, captains)| Walk::Aligned {
        range: range_index,
        start: positions.start,
        len: positions.len(),
        captains,
    });

    // A match of this range past the aligned prefix is in the damaged suffix of every
    // required member. Walking one of them is enough: the shortest, the first in child order
    // on ties.
    let owner = *required_parts()
        .min_by_key(|member| member.ring.len - aligned_len)
        .expect("cover sets require at least two members");

    // Another member reads its component directly only where it and the owner both
    // trust the position. Past the owner's trust, nothing tells where they hold its entity.
    let damaged = pieces(aligned_len..owner.ring.len, trusts(), move |position| {
        if position < owner.trust {
            trusting(range, position)
        } else {
            1 << owner.child
        }
    })
    .map(move |(positions, captains)| Walk::Damaged {
        range: range_index,
        owner: owner.child,
        start: positions.start,
        len: positions.len(),
        captains,
    });

    aligned.chain(damaged)
}

/// Returns the walks over the unevaluated regions of the members `required` flags,
/// in emission order.
///
/// `members` lists every child taking part in a walk, in child order.
fn unevaluated_walks(required: u32, members: &[Member]) -> impl Iterator<Item = Walk> {
    let unevaluated_len = |member: &Member| member.len - member.unevaluated_start;

    // Each required member walks its unevaluated region. A walk rejects what the walks before
    // it covered, so later walks check more regions: the longest go first, in child order on ties.
    // We pick the next one among those left each time, there's only a handful of them.
    let mut left = required;
    let ordered = core::iter::from_fn(move || {
        // `min_by_key` keeps the first of equal keys, so ties go in child order.
        let member = members
            .iter()
            .filter(|member| left & (1 << member.child) != 0)
            .min_by_key(|&member| Reverse(unevaluated_len(member)))?;

        left &= !(1 << member.child);
        Some(member)
    });

    ordered
        .filter(move |&member| unevaluated_len(member) > 0)
        .map(move |member| Walk::Unevaluated {
            owner: member.child,
            start: member.unevaluated_start,
            len: unevaluated_len(member),
        })
}

/// Returns the members of `range` trusting logical position `position`, bit `i` for child `i`.
fn trusting(range: &CoverRange, position: usize) -> u32 {
    range
        .members
        .iter()
        .filter(|member| position < member.trust)
        .fold(0, |mask, member| mask | 1 << member.child)
}

/// Splits `positions` into pieces with the same captains, `captains_at` returning the captains
/// of a position.
///
/// The captains can only change at one of `bounds`. Empty positions give no piece.
fn pieces(
    positions: Range<usize>,
    bounds: impl Iterator<Item = usize> + Clone,
    captains_at: impl Fn(usize) -> u32,
) -> impl Iterator<Item = (Range<usize>, u32)> {
    let end = positions.end;
    let mut next = positions.start;

    // Bounds come unsorted, we look for the closest one each time instead of sorting them.
    // Ranges have a handful of members, so a handful of bounds.
    let next_bound = move |position: usize| {
        bounds
            .clone()
            .filter(|&bound| position < bound)
            .fold(end, usize::min)
    };

    core::iter::from_fn(move || {
        if next >= end {
            return None;
        }

        let start = next;
        let captains = captains_at(start);

        // A bound where nothing changes doesn't split anything, the piece goes on past it.
        let mut piece_end = next_bound(start);
        while piece_end < end && captains_at(piece_end) == captains {
            piece_end = next_bound(piece_end);
        }

        next = piece_end;
        Some((start..piece_end, captains))
    })
}
