//! Construction-only choice between an ordinary walk and walks over group ranges.
//!
//! While a query is built, each node reports what it knows about grouping in a
//! [`ConstructionMetadata`]. A window's points to its storage's group metadata, nothing is copied:
//! the storage stays borrowed while the query is built.
//! It travels next to the node's entities, never inside them.\
//! A `Mixed` hands its immediate children to [`select`], which ranks the ordinary captains
//! and estimates its grouped [`Alternative`]s: walks over the ranges containing each set of
//! overlapping groups its window children fully hold (see [`cover_sets`]), and the
//! alternatives its nested `Mixed` children reported, driven through them.\
//! Everything is kept in a [`Selection`] until the query decides which walk drives.
//!
//! Nothing here reads entities. Estimates only use group metadata, and the metadata is
//! released once the iterator is built.\
//! Constructors always install their ordinary walk. Once the whole query is built,
//! [`install_winner`] swaps in its root's cheapest alternative when it beats the ordinary walk,
//! [`regions::grouped_entities`] turning it into segments.
pub(crate) mod regions;
#[cfg(test)]
mod tests;

use crate::component::Component;
use crate::iter::mixed::NON_CAPTAIN_FACTOR;
use crate::iter::{EntityCursor, RawEntityIdAccess, Shiperator};
use crate::sparse_set::{GroupRange, Groups, Ring, SparseSet};
use crate::storage::StorageId;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Reverse;
use core::fmt;
use core::ops::Range;
use core::ptr::NonNull;

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
        ConstructionMetadata(NodeMetadata::window(sparse_set))
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
    /// Returns the metadata of a plain window over `sparse_set`.
    #[inline]
    pub(crate) fn window<T: Component>(sparse_set: &SparseSet<T>) -> NodeMetadata {
        NodeMetadata::of_storage(&sparse_set.groups, sparse_set.dense.len())
    }

    /// Returns the metadata of a plain window over a storage of `len` components with `groups`.
    ///
    /// The returned metadata points to `groups`, see [`WindowHandle::get`].
    fn of_storage(groups: &Groups, len: usize) -> NodeMetadata {
        // Storages without group only pay this check.
        if !groups.has_ranges() {
            return NodeMetadata::Ungrouped;
        }
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
    /// Returns the group metadata this handle points to.
    ///
    /// ### Safety
    ///
    /// The storage this handle was taken from must stay borrowed for `'m`, with its group
    /// metadata unmodified and no exclusive reference to it created.
    pub(crate) unsafe fn get<'m>(self) -> WindowMetadata<'m> {
        // SAFE: Our caller keeps the storage borrowed for 'm, the pointer we took from its
        // `Groups` stays valid and nothing modifies them meanwhile.
        let groups = unsafe { self.groups.as_ref() };

        WindowMetadata {
            storage_id: groups
                .storage_id
                .expect("handles are only taken for storages recording their id"),
            len: self.len,
            unevaluated_start: groups.unevaluated_start,
            ranges: &groups.ranges,
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
    /// Id of the storage, as its own group metadata records it.
    ///
    /// Thread-local storages have ids distinct from their `Send + Sync` counterparts.
    pub(crate) storage_id: StorageId,
    /// Number of components in the storage.
    pub(crate) len: usize,
    /// Physical index where the unevaluated region starts.
    pub(crate) unevaluated_start: usize,
    /// Every range of the storage, in physical order.
    pub(crate) ranges: &'m [GroupRange],
}

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
    /// The first entities the child walks when it drives.
    ///
    /// For a window over a grouped storage, every entity of that storage.\
    /// Grouped walks take their segments from it.
    pub(crate) entities: EntityCursor,
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
            NodeMetadata::Window(handle) => NodeMetadata::Window(unsafe { handle.get() }),
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
            entities: entities.cursor,
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
    fn ordinary_work(&self) -> Option<u64> {
        match &self.metadata {
            NodeMetadata::Mixed { ordinary_work, .. } => *ordinary_work,
            NodeMetadata::GroupedMixed(selection) => selection.ordinary.work,
            _ => walk_work(self.candidates?, 0),
        }
    }

    /// Moves out the grouped alternatives this child reported, only a nested `Mixed` has some.
    fn take_alternatives(&mut self) -> Vec<Alternative> {
        match &mut self.metadata {
            NodeMetadata::GroupedMixed(selection) => core::mem::take(&mut selection.alternatives),
            _ => Vec::new(),
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
#[derive(Clone, Debug)]
pub(crate) struct Alternative {
    /// Number of candidates walked, an upper bound of the matches since lookups can reject.
    pub(crate) candidates: usize,
    /// Estimated work for the `Mixed` reporting it, the lookups of all its children included.
    /// See [`walk_work`].
    pub(crate) work: u64,
    pub(crate) route: Route,
}

/// Where an [`Alternative`]'s group is, relative to the `Mixed` reporting it.
#[derive(Clone, Debug)]
pub(crate) enum Route {
    /// Walks over a group the `Mixed`'s immediate window children fully hold.
    Local(GroupedCandidate),
    /// The nested `Mixed` at position `child` drives through one of its own alternatives,
    /// the other children are looked up.
    ///
    /// The nested `Mixed` keeps its own captain mask and coverage, nothing is flattened.
    Nested {
        child: usize,
        alternative: Box<Alternative>,
    },
}

/// Walks covering every match of a query through the ranges of one of its cover sets.
///
/// This is the region recipe: the walks are listed in the order they're emitted
/// and the estimate is computed from these same walks.
#[derive(Clone, Debug)]
pub(crate) struct GroupedCandidate {
    /// Children holding the required members, bit `i` for child `i`, see [`CoverSet`].
    pub(crate) required: u32,
    /// The ranges walks refer to, by position.
    pub(crate) ranges: Vec<CoverRange>,
    /// Every child taking part in a walk, in child order.
    pub(crate) members: Vec<Member>,
    /// Walks in emission order, empty ones are left out.
    pub(crate) walks: Vec<Walk>,
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
fn walk_work(candidates: usize, lookups: usize) -> Option<u64> {
    let per_candidate = CANDIDATE_WORK.checked_add(SAILOR_WORK.checked_mul(lookups as u64)?)?;

    u64::try_from(candidates).ok()?.checked_mul(per_candidate)
}

/// Returns the estimated work of looking up `lookups` components for each of `candidates`
/// candidates someone else walks, see [`walk_work`].
///
/// Returns `None` when the result doesn't fit in a `u64`.
fn lookup_work(candidates: usize, lookups: usize) -> Option<u64> {
    let per_candidate = SAILOR_WORK.checked_mul(lookups as u64)?;

    u64::try_from(candidates).ok()?.checked_mul(per_candidate)
}

/// Ranks the alternatives of a `Mixed` with these immediate `children`.
///
/// `children` must have at least one child able to captain and at most one unable to sail,
/// constructors reject other queries.\
/// The alternatives nested `Mixed` children reported move into the returned selection.
pub(crate) fn select(children: &mut [Child<'_>]) -> Selection {
    let lookups = children.iter().map(Child::lookups).sum();
    let ordinary = ordinary_candidate(children, lookups);

    let filtered = children
        .iter()
        .any(|child| matches!(child.metadata, NodeMetadata::Filtered));
    // A child that can't sail has to drive, no group can replace it.
    let has_forced_captain = children.iter().any(|child| !child.can_sailor);

    let alternatives = if filtered || has_forced_captain {
        Vec::new()
    } else {
        alternatives(children, lookups)
    };

    Selection {
        ordinary,
        alternatives,
        lookups,
        filtered,
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
pub(crate) fn install_winner<S: Shiperator>(
    root: &mut S,
    entities: RawEntityIdAccess,
    metadata: ConstructionMetadata,
) -> RawEntityIdAccess {
    // Only a `Mixed` reports alternatives. Wrappers either forward their child's report
    // untouched or mark it filtered, so these are about `root`'s own immediate children.
    let NodeMetadata::GroupedMixed(selection) = &metadata.0 else {
        return entities;
    };
    let Some(winner) = selection.winner() else {
        return entities;
    };

    // `root` gets the first grouped segment's state, overwriting the ordinary one.
    // The ordinary entities, and any plan they embed, are dropped with `entities`.
    regions::grouped_entities(root, winner)
}

/// Returns the ordinary walk of `children`, which read `lookups` components as sailors.
fn ordinary_candidate(children: &[Child<'_>], lookups: usize) -> OrdinaryCandidate {
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
fn ordinary_captain(children: &[Child<'_>]) -> usize {
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

/// Returns every grouped alternative of `children`, which read `lookups` components as sailors.
///
/// Alternatives are ordered by the first child they involve: a cover set by its first
/// required member, a nested `Mixed`'s alternatives by its position.\
/// Alternatives whose estimate doesn't fit in a `u64` can't beat anything and are left out.
fn alternatives(children: &mut [Child<'_>], lookups: usize) -> Vec<Alternative> {
    let local: Vec<GroupedCandidate> = cover_sets(children)
        .into_iter()
        .map(|set| GroupedCandidate::new(children, set))
        .collect();
    let mut local = local.into_iter().peekable();
    let mut alternatives = Vec::new();

    for (position, child) in children.iter_mut().enumerate() {
        // Only a window can start a cover set and only a nested `Mixed` has alternatives,
        // at most one of these two adds something.
        // Cover sets come ordered by their first required member.
        while let Some(candidate) =
            local.next_if(|candidate| candidate.required.trailing_zeros() as usize == position)
        {
            alternatives.extend(Alternative::local(candidate, lookups));
        }

        // Groups don't cross `Mixed` boundaries: a nested `Mixed` is never a member of our
        // groups and we drive through its alternatives as a whole, its masks stay its own.
        let sailor_lookups = lookups - child.lookups();
        for alternative in child.take_alternatives() {
            alternatives.extend(Alternative::nested(position, alternative, sailor_lookups));
        }
    }

    alternatives
}

/// Members a grouped alternative of a `Mixed` has to cover, and the ranges it can walk.
///
/// Built from the group metadata of the `Mixed`'s immediate window children alone.
#[derive(Clone, Debug)]
pub(crate) struct CoverSet {
    /// Children holding the required members, bit `i` for child `i`.
    ///
    /// They're the union of overlapping groups the children fully hold. Only they establish
    /// completeness: every match has a component in each of them.
    pub(crate) required: u32,
    /// Every range whose group contains all required members, in the first required child's
    /// range order.
    pub(crate) ranges: Vec<CoverRange>,
}

/// A range whose group contains all required members of a [`CoverSet`].
#[derive(Clone, Debug)]
pub(crate) struct CoverRange {
    /// Canonical identity retained for topology assertions in tests.
    #[cfg(test)]
    pub(crate) group: Arc<[StorageId]>,
    /// Each child holding a member of the group, in child order, with its part of the range.
    ///
    /// The required members are always there. Other members the query holds can come along,
    /// members the query doesn't borrow never do.
    pub(crate) members: Vec<RangeMember>,
}

/// A child's part of a [`CoverRange`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct RangeMember {
    /// Position of the child among the `Mixed`'s children.
    pub(crate) child: usize,
    pub(crate) ring: Ring,
    /// Length of the child's logical prefix of the range unchanged since the last regroup.
    pub(crate) trust: usize,
}

/// Returns the cover sets of `children`, one per set of overlapping groups they fully hold,
/// ordered by their first child.
///
/// A group is fully held when each of its members is held by exactly one window child.
/// With an `(A, B)` query and only an ABC group, matches without C sit outside the group's
/// ranges and only an ordinary walk finds them.\
/// Groups sharing a member are joined: with AB and AC, an entity holding A, B and C is in
/// an ABC union, not in AB or AC. So an `(A, B, C)` query covers ABC, whether or not the
/// union exists yet, and doesn't walk AB or AC.\
/// Disjoint groups give distinct sets, they compete as distinct alternatives.
pub(crate) fn cover_sets(children: &[Child<'_>]) -> Vec<CoverSet> {
    // Each fully held group as the children holding its members, deduplicated by pointer.
    // Every member lists the group, the first one we meet is enough.
    let mut seen: Vec<&Arc<[StorageId]>> = Vec::new();
    let mut held: Vec<u32> = Vec::new();
    for window in children.iter().filter_map(Child::window) {
        for range in window.ranges {
            if seen.iter().any(|group| Arc::ptr_eq(group, &range.group)) {
                continue;
            }
            seen.push(&range.group);

            if let Some(mask) = member_mask(children, &range.group) {
                held.push(mask);
            }
        }
    }

    // We join groups sharing a member until no two sets overlap.
    let mut required: Vec<u32> = Vec::new();
    for mut mask in held {
        // Merging can make a set overlap others it didn't before, so we absorb them all.
        while let Some(position) = required.iter().position(|&other| other & mask != 0) {
            mask |= required.swap_remove(position);
        }
        required.push(mask);
    }
    // The lowest bit is the first child, the order doesn't depend on how groups were found.
    required.sort_unstable_by_key(|mask| mask.trailing_zeros());

    required
        .into_iter()
        .map(|required| CoverSet {
            required,
            ranges: containing_ranges(children, required),
        })
        .collect()
}

/// Returns the children holding `group`'s members, bit `i` for child `i`,
/// `None` unless each member is held by exactly one window child.
fn member_mask(children: &[Child<'_>], group: &[StorageId]) -> Option<u32> {
    group.iter().try_fold(0, |mask, &storage_id| {
        let child = window_child(children, storage_id)?;

        Some(mask | 1u32.checked_shl(u32::try_from(child).ok()?)?)
    })
}

/// Returns the position of the window child over the storage with id `storage_id`,
/// `None` when no child or several children hold it.
///
/// A storage held by several children, like A in `(&a, &a, &b)`, is ambiguous and we don't
/// pick which child would drive.
fn window_child(children: &[Child<'_>], storage_id: StorageId) -> Option<usize> {
    let mut holders = children
        .iter()
        .enumerate()
        .filter(|(_, child)| {
            child
                .window()
                .is_some_and(|window| window.storage_id == storage_id)
        })
        .map(|(position, _)| position);

    let child = holders.next()?;
    holders.next().is_none().then_some(child)
}

/// Returns every range whose group contains all members `required` flags.
fn containing_ranges(children: &[Child<'_>], required: u32) -> Vec<CoverRange> {
    let storage_of = |child: usize| {
        children[child]
            .window()
            .expect("required members are window children")
            .storage_id
    };
    let required_storages: Vec<StorageId> = (0..children.len())
        .filter(|&child| required & (1 << child) != 0)
        .map(storage_of)
        .collect();

    // A group containing every required member is attached to each of them,
    // the first one's ranges list all of them.
    let first = required.trailing_zeros() as usize;
    let first_window = children[first]
        .window()
        .expect("required members are window children");

    first_window
        .ranges
        .iter()
        .filter(|range| {
            required_storages
                .iter()
                .all(|storage_id| range.group.binary_search(storage_id).is_ok())
        })
        .map(|range| CoverRange {
            #[cfg(test)]
            group: range.group.clone(),
            members: range_members(children, &range.group),
        })
        .collect()
}

/// Returns each child holding a member of `group` alone, in child order, with its part of `group`'s range.
fn range_members(children: &[Child<'_>], group: &Arc<[StorageId]>) -> Vec<RangeMember> {
    let mut members: Vec<RangeMember> = group
        .iter()
        .filter_map(|&storage_id| window_child(children, storage_id))
        .map(|child| {
            let range = children[child]
                .window()
                .expect("window_child only returns windows")
                .ranges
                .iter()
                .find(|range| Arc::ptr_eq(&range.group, group))
                .expect("every member holds a range for its group");

            RangeMember {
                child,
                ring: range.ring(),
                trust: range.trust,
            }
        })
        .collect();

    // Group ids are sorted by storage, we want the members in child order.
    members.sort_unstable_by_key(|member| member.child);

    members
}

impl Alternative {
    /// Returns the alternative walking `candidate`'s walks, for a `Mixed` whose children
    /// read `lookups` components as sailors.
    ///
    /// Returns `None` when the estimate doesn't fit in a `u64`.
    fn local(candidate: GroupedCandidate, lookups: usize) -> Option<Alternative> {
        let mut candidates = 0usize;
        let mut work = 0u64;

        for walk in &candidate.walks {
            // Captains are member windows, each one would otherwise be a single lookup.
            let sailor_lookups = lookups - walk.captains().count_ones() as usize;

            candidates = candidates.checked_add(walk.len())?;
            work = work.checked_add(walk_work(walk.len(), sailor_lookups)?)?;
        }

        Some(Alternative {
            candidates,
            work,
            route: Route::Local(candidate),
        })
    }

    /// Returns the alternative where the child at position `child` drives through `alternative`
    /// and the other children read `sailor_lookups` components for each of its candidates.
    ///
    /// Returns `None` when the estimate doesn't fit in a `u64`.
    fn nested(
        child: usize,
        alternative: Alternative,
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
                alternative: Box::new(alternative),
            },
        })
    }

    /// Returns the group walks this alternative ends up driving, through any nested `Mixed`.
    #[cfg(test)]
    pub(crate) fn group(&self) -> &GroupedCandidate {
        match &self.route {
            Route::Local(candidate) => candidate,
            Route::Nested { alternative, .. } => alternative.group(),
        }
    }
}

impl GroupedCandidate {
    /// Returns the walks covering `set`'s required members, for a `Mixed` with these `children`.
    fn new(children: &[Child<'_>], set: CoverSet) -> GroupedCandidate {
        // Required members walk their unevaluated regions, any member of a range can drive part of it.
        let member_mask = set
            .ranges
            .iter()
            .flat_map(|range| &range.members)
            .fold(set.required, |mask, member| mask | 1 << member.child);
        let members: Vec<Member> = mask_children(member_mask)
            .map(|child| {
                let window = children[child]
                    .window()
                    .expect("members are window children");

                Member {
                    child,
                    len: window.len,
                    unevaluated_start: window.unevaluated_start,
                    entities: children[child].entities,
                }
            })
            .collect();

        let walks = walks(set.required, &set.ranges, &members);

        GroupedCandidate {
            required: set.required,
            ranges: set.ranges,
            members,
            walks,
        }
    }
}

/// Returns the walks covering the members `required` flags through `ranges`, in emission order.
///
/// `members` lists every child taking part in a walk. Empty walks are left out.
fn walks(required: u32, ranges: &[CoverRange], members: &[Member]) -> Vec<Walk> {
    let mut walks = Vec::new();

    for (range_index, range) in ranges.iter().enumerate() {
        let required_parts = || {
            range
                .members
                .iter()
                .filter(|member| required & (1 << member.child) != 0)
        };
        let trusts = || range.members.iter().map(|member| member.trust);

        // Every required member holds the same entities over the logical prefix all of them trust.
        let aligned_len = required_parts()
            .map(|member| member.trust)
            .min()
            .expect("cover sets require at least two members");

        // Members trusting the same positions hold the same entities there, so the other
        // members join the required ones up to their own trust. A member trusting less
        // doesn't shorten the prefix, it's looked up past its trust.
        for (positions, captains) in pieces(0..aligned_len, trusts(), |position| {
            trusting(range, position)
        }) {
            walks.push(Walk::Aligned {
                range: range_index,
                start: positions.start,
                len: positions.len(),
                captains,
            });
        }

        // A match of this range past the aligned prefix is in the damaged suffix of every
        // required member. Walking one of them is enough: the shortest, the first in child order
        // on ties.
        let owner = required_parts()
            .min_by_key(|member| member.ring.len - aligned_len)
            .expect("cover sets require at least two members");

        // Another member reads its component directly only where it and the owner both
        // trust the position. Past the owner's trust, nothing tells where they hold its entity.
        for (positions, captains) in pieces(aligned_len..owner.ring.len, trusts(), |position| {
            if position < owner.trust {
                trusting(range, position)
            } else {
                1 << owner.child
            }
        }) {
            walks.push(Walk::Damaged {
                range: range_index,
                owner: owner.child,
                start: positions.start,
                len: positions.len(),
                captains,
            });
        }
    }

    // Each required member walks its unevaluated region. A walk rejects what the walks before
    // it covered, so later walks check more regions: the longest go first, in child order on ties.
    let mut unevaluated: Vec<&Member> = members
        .iter()
        .filter(|member| required & (1 << member.child) != 0)
        .collect();
    unevaluated.sort_by_key(|member| Reverse(member.len - member.unevaluated_start));

    for member in unevaluated {
        let len = member.len - member.unevaluated_start;

        if len > 0 {
            walks.push(Walk::Unevaluated {
                owner: member.child,
                start: member.unevaluated_start,
                len,
            });
        }
    }

    walks
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
    bounds: impl Iterator<Item = usize>,
    captains_at: impl Fn(usize) -> u32,
) -> Vec<(Range<usize>, u32)> {
    let mut ends: Vec<usize> = bounds
        .filter(|&bound| positions.start < bound && bound < positions.end)
        .collect();
    ends.push(positions.end);
    ends.sort_unstable();
    ends.dedup();

    let mut pieces: Vec<(Range<usize>, u32)> = Vec::new();
    let mut start = positions.start;
    for end in ends {
        if start == end {
            continue;
        }

        let captains = captains_at(start);
        match pieces.last_mut() {
            // A bound where nothing changes doesn't split anything.
            Some((last, last_captains)) if *last_captains == captains => last.end = end,
            _ => pieces.push((start..end, captains)),
        }

        start = end;
    }

    pieces
}
