//! Construction-only choice between an ordinary walk and walks over group ranges.
//!
//! While a query is built, each node reports what it knows about grouping in a
//! [`NodeMetadata`], read from its storages while they're borrowed.\
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

/// What a query node knows about grouping, reported while its storages are borrowed.
///
/// It only exists while the query is built, iterators don't keep it.
#[derive(Clone, Default)]
pub(crate) enum NodeMetadata {
    /// Nothing that could help or prevent grouping.
    ///
    /// A window over a storage without group range or an entity slice.
    #[default]
    Ungrouped,
    /// A plain window over a storage holding group ranges.
    Window(WindowMetadata),
    /// An `Optional`, `Not`, tracking filter or `Or`, or a query containing one.
    ///
    /// These queries keep ordinary selection all the way up to the root.
    Filtered,
    /// A `Mixed` without filter, with its ordinary walk and its grouped alternatives.
    ///
    /// Its parent can drive through any of these alternatives, only the query's root
    /// installs one, see [`install_winner`].
    Mixed(Selection),
}

impl NodeMetadata {
    /// Returns the metadata of a plain window over `sparse_set`.
    #[inline]
    pub(crate) fn window<T: Component>(sparse_set: &SparseSet<T>) -> NodeMetadata {
        NodeMetadata::of_storage(&sparse_set.groups, sparse_set.dense.len())
    }

    /// Returns the metadata of a plain window over a storage of `len` components with `groups`.
    fn of_storage(groups: &Groups, len: usize) -> NodeMetadata {
        // Storages without group only pay this check.
        if !groups.has_ranges() {
            return NodeMetadata::Ungrouped;
        }
        let Some(storage_id) = groups.storage_id else {
            return NodeMetadata::Ungrouped;
        };

        let window = WindowMetadata {
            storage_id,
            len,
            unevaluated_start: groups.unevaluated_start,
            ranges: groups
                .ranges
                .iter()
                .map(|range| RangeMetadata {
                    group: range.group.clone(),
                    ring: range.ring(),
                    trust: range.trust,
                })
                .collect(),
        };

        // Walks are placed and estimated from these bounds without further checks,
        // so a layout outside the storage stays ordinary instead of being trusted.
        if !window.is_valid() {
            return NodeMetadata::Ungrouped;
        }

        NodeMetadata::Window(window)
    }
}

/// Group metadata of a plain window's storage, copied while the storage is borrowed.
///
/// ```text
/// [ Group range(s) ][ Ungrouped ][ Unevaluated ]
/// ```
#[derive(Clone, Debug)]
pub(crate) struct WindowMetadata {
    /// Id of the storage, as its own group metadata records it.
    ///
    /// Thread-local storages have ids distinct from their `Send + Sync` counterparts.
    pub(crate) storage_id: StorageId,
    /// Number of components in the storage.
    pub(crate) len: usize,
    /// Physical index where the unevaluated region starts.
    pub(crate) unevaluated_start: usize,
    /// Every range of the storage, in physical order.
    pub(crate) ranges: Vec<RangeMetadata>,
}

/// One range of a plain window's storage.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct RangeMetadata {
    /// Sorted member ids of the range's group, compared by pointer.
    pub(crate) group: Arc<[StorageId]>,
    pub(crate) ring: Ring,
    /// Length of the range's logical prefix unchanged since the last regroup.
    pub(crate) trust: usize,
}

impl WindowMetadata {
    /// Returns `true` when the ranges tile the start of the storage, each one trusting at most
    /// its length, followed by the ungrouped and unevaluated regions inside the storage.
    fn is_valid(&self) -> bool {
        let mut grouped_end = 0usize;
        for range in &self.ranges {
            if range.ring.start != grouped_end || range.trust > range.ring.len {
                return false;
            }

            let Some(end) = grouped_end.checked_add(range.ring.len) else {
                return false;
            };
            grouped_end = end;
        }

        grouped_end <= self.unevaluated_start && self.unevaluated_start <= self.len
    }
}

/// What the selector needs to know about an immediate child of a `Mixed`.
pub(crate) struct Child {
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
    pub(crate) metadata: NodeMetadata,
}

impl Child {
    /// Describes a child from its capabilities, sail time and `entities`.
    ///
    /// The metadata moves out of `entities`, it doesn't follow them into the iterator.
    #[inline]
    pub(crate) fn new(
        can_captain: bool,
        can_sailor: bool,
        sail_time: usize,
        entities: &mut RawEntityIdAccess,
    ) -> Child {
        Child {
            can_captain,
            can_sailor,
            sail_time,
            candidates: entities.candidate_count(),
            entities: entities.cursor,
            metadata: core::mem::take(&mut entities.metadata),
        }
    }

    /// Returns the group metadata of a plain window child over a grouped storage.
    fn window(&self) -> Option<&WindowMetadata> {
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
            NodeMetadata::Mixed(selection) => selection.lookups,
            _ => 1,
        }
    }

    /// Returns the estimated work of this child's ordinary walk, without its parent's lookups.
    ///
    /// Returns `None` when it doesn't fit in a `u64`.
    fn ordinary_work(&self) -> Option<u64> {
        match &self.metadata {
            NodeMetadata::Mixed(selection) => selection.ordinary.work,
            _ => walk_work(self.candidates?, 0),
        }
    }

    /// Moves out the grouped alternatives this child reported, only a nested `Mixed` has some.
    fn take_alternatives(&mut self) -> Vec<Alternative> {
        match &mut self.metadata {
            NodeMetadata::Mixed(selection) => core::mem::take(&mut selection.alternatives),
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
pub(crate) fn select(children: &mut [Child]) -> Selection {
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
    pub(crate) fn into_metadata(self) -> NodeMetadata {
        if self.filtered {
            NodeMetadata::Filtered
        } else {
            NodeMetadata::Mixed(self)
        }
    }
}

/// Returns the entities driving a whole query, installing its cheapest alternative on `root`
/// when it beats its ordinary walk.
///
/// `root` and `entities` are what the query's constructor returned, `root` still in the state
/// it built.\
/// The returned entities never carry metadata, the losing alternatives are dropped here.
///
/// Only the root may call this. Each nested `Mixed` keeps the ordinary walk its constructor
/// installed until then: its parent estimated its own alternatives with that walk and may
/// drive through it.
pub(crate) fn install_winner<S: Shiperator>(
    root: &mut S,
    entities: RawEntityIdAccess,
) -> RawEntityIdAccess {
    // Only a `Mixed` reports a selection. Wrappers either forward their child's report
    // untouched or mark it filtered, so this one is about `root`'s own immediate children.
    let NodeMetadata::Mixed(selection) = &entities.metadata else {
        return entities.without_metadata();
    };
    let Some(winner) = selection.winner() else {
        return entities.without_metadata();
    };

    // `root` gets the first grouped segment's state, overwriting the ordinary one.
    // The ordinary entities, and any plan they embed, are dropped with `entities`.
    regions::grouped_entities(root, winner)
}

/// Returns the ordinary walk of `children`, which read `lookups` components as sailors.
fn ordinary_candidate(children: &[Child], lookups: usize) -> OrdinaryCandidate {
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
fn ordinary_captain(children: &[Child]) -> usize {
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
fn alternatives(children: &mut [Child], lookups: usize) -> Vec<Alternative> {
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
pub(crate) fn cover_sets(children: &[Child]) -> Vec<CoverSet> {
    // Each fully held group as the children holding its members, deduplicated by pointer.
    // Every member lists the group, the first one we meet is enough.
    let mut seen: Vec<&Arc<[StorageId]>> = Vec::new();
    let mut held: Vec<u32> = Vec::new();
    for window in children.iter().filter_map(Child::window) {
        for range in &window.ranges {
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
fn member_mask(children: &[Child], group: &[StorageId]) -> Option<u32> {
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
fn window_child(children: &[Child], storage_id: StorageId) -> Option<usize> {
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
fn containing_ranges(children: &[Child], required: u32) -> Vec<CoverRange> {
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
fn range_members(children: &[Child], group: &Arc<[StorageId]>) -> Vec<RangeMember> {
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
                ring: range.ring,
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
    fn new(children: &[Child], set: CoverSet) -> GroupedCandidate {
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

#[cfg(test)]
mod grouping_selection_arithmetic {
    use super::*;
    use crate::sparse_set::GroupRange;
    use alloc::vec;

    fn group() -> Arc<[StorageId]> {
        Arc::from([StorageId::Custom(0), StorageId::Custom(1)])
    }

    /// A window child holding the `member`th member of `group`,
    /// with a fully trusted range of `range_len` in a storage of `len` components.
    fn member(group: &Arc<[StorageId]>, member: usize, range_len: usize, len: usize) -> Child {
        Child {
            can_captain: true,
            can_sailor: true,
            sail_time: len,
            candidates: Some(len),
            entities: EntityCursor::empty(),
            metadata: NodeMetadata::Window(WindowMetadata {
                storage_id: group[member],
                len,
                unevaluated_start: range_len,
                ranges: vec![RangeMetadata {
                    group: group.clone(),
                    ring: Ring {
                        start: 0,
                        len: range_len,
                        shift: 0,
                    },
                    trust: range_len,
                }],
            }),
        }
    }

    /// Returns the metadata of a child made by `member`.
    fn window_mut(child: &mut Child) -> &mut WindowMetadata {
        match &mut child.metadata {
            NodeMetadata::Window(window) => window,
            _ => unreachable!(),
        }
    }

    /// Lays the storage of a child made by `member` out with an unrotated range of `range_len`
    /// trusting `trust`, then the unevaluated region from `unevaluated_start` to `len`.
    fn set_regions(
        child: &mut Child,
        range_len: usize,
        trust: usize,
        unevaluated_start: usize,
        len: usize,
    ) {
        let window = window_mut(child);
        window.ranges[0].ring = Ring {
            start: 0,
            len: range_len,
            shift: 0,
        };
        window.ranges[0].trust = trust;
        window.unevaluated_start = unevaluated_start;
        window.len = len;
    }

    /// A window child of `len` components without group metadata.
    fn ungrouped(len: usize) -> Child {
        Child {
            can_captain: true,
            can_sailor: true,
            sail_time: len,
            candidates: Some(len),
            entities: EntityCursor::empty(),
            metadata: NodeMetadata::Ungrouped,
        }
    }

    /// Asserts that a walk costs `candidates * (2 + sailors)`.
    #[test]
    fn walk_formula() {
        assert_eq!(walk_work(0, 3), Some(0));
        assert_eq!(walk_work(10, 0), Some(20));
        assert_eq!(walk_work(10, 2), Some(40));
    }

    /// Asserts that walks past `u64` are reported as unrepresentable instead of wrapping,
    /// right at the boundary.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn walk_overflow() {
        assert_eq!(walk_work(usize::MAX / 2, 0), Some(u64::MAX - 1));
        assert_eq!(walk_work(usize::MAX / 2 + 1, 0), None);
        assert_eq!(walk_work(usize::MAX / 3, 1), Some(u64::MAX));
        assert_eq!(walk_work(usize::MAX / 3 + 1, 1), None);
    }

    /// Asserts that a grouped alternative whose walks sum past `u64` is dropped
    /// instead of wrapping into a cheap estimate, the small ordinary walk stays.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn huge_grouped() {
        let group = group();
        let mut children = [
            member(&group, 0, usize::MAX / 2, usize::MAX),
            member(&group, 1, usize::MAX / 2, usize::MAX),
            ungrouped(5),
        ];

        let selection = select(&mut children);

        assert!(selection.alternatives.is_empty());
        assert_eq!(selection.ordinary.captain, 2);
        // 5 candidates, 2 sailors.
        assert_eq!(selection.ordinary.work, Some(20));
    }

    /// Asserts that a single walk past `u64` sinks a grouped alternative,
    /// even when the walks before it fit.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn huge_unevaluated() {
        let group = group();
        let a = member(&group, 0, 3, usize::MAX);
        let b = member(&group, 1, 3, 3);

        assert!(select(&mut [a, b]).alternatives.is_empty());
    }

    /// Asserts that an ordinary walk whose estimate doesn't fit in a `u64` still beats
    /// a small grouped alternative, unrepresentable comparisons keep ordinary.
    #[test]
    fn huge_ordinary() {
        let group = group();
        // The smallest sail time drives, but its follow-ups overflow the candidate count.
        let mut overflowing = ungrouped(1);
        overflowing.candidates = None;
        let mut children = [
            overflowing,
            member(&group, 0, 3, 3),
            member(&group, 1, 3, 3),
        ];

        let selection = select(&mut children);

        assert_eq!(selection.ordinary.captain, 0);
        assert_eq!(selection.ordinary.work, None);
        assert_eq!(selection.cheapest().unwrap().work, 9);
        assert!(selection.winner().is_none());
    }

    /// Asserts that a fully aligned group is a single aligned walk where the members'
    /// lookups are avoided.
    #[test]
    fn aligned_recipe() {
        let group = group();
        let mut children = [
            member(&group, 0, 4, 4),
            member(&group, 1, 4, 4),
            ungrouped(9),
        ];

        let selection = select(&mut children);
        let alternative = selection.cheapest().unwrap();

        assert_eq!(
            alternative.group().walks,
            [Walk::Aligned {
                range: 0,
                start: 0,
                len: 4,
                captains: 0b011
            }]
        );
        assert_eq!(alternative.group().required, 0b011);
        // 4 candidates, 1 sailor.
        assert_eq!((alternative.candidates, alternative.work), (4, 12));
    }

    /// Asserts that partial trust gives an aligned prefix, a single damaged walk owned by
    /// the member with the shortest suffix, then unevaluated walks longest first.
    #[test]
    fn damaged_recipe() {
        let group = group();
        let mut a = member(&group, 0, 6, 9);
        let mut b = member(&group, 1, 5, 12);
        set_regions(&mut a, 6, 2, 7, 9);
        set_regions(&mut b, 5, 3, 6, 12);

        let selection = select(&mut [a, b]);
        let alternative = selection.cheapest().unwrap();

        assert_eq!(
            alternative.group().walks,
            [
                Walk::Aligned {
                    range: 0,
                    start: 0,
                    len: 2,
                    captains: 0b11
                },
                // B trusts position 2 but A doesn't, B is its only captain.
                Walk::Damaged {
                    range: 0,
                    owner: 1,
                    start: 2,
                    len: 3,
                    captains: 0b10
                },
                Walk::Unevaluated {
                    owner: 1,
                    start: 6,
                    len: 6
                },
                Walk::Unevaluated {
                    owner: 0,
                    start: 7,
                    len: 2
                },
            ]
        );
        // 2 aligned candidates without sailor, then 11 candidates with one sailor each.
        assert_eq!(alternative.work, 2 * 2 + 11 * 3);
    }

    /// Asserts that equal damaged suffixes and equal unevaluated regions are owned
    /// in child order.
    #[test]
    fn recipe_ties() {
        let group = group();
        let mut a = member(&group, 0, 4, 6);
        let mut b = member(&group, 1, 4, 6);
        window_mut(&mut a).ranges[0].trust = 1;
        window_mut(&mut b).ranges[0].trust = 1;

        let selection = select(&mut [a, b]);

        assert_eq!(
            selection.cheapest().unwrap().group().walks,
            [
                Walk::Aligned {
                    range: 0,
                    start: 0,
                    len: 1,
                    captains: 0b11
                },
                Walk::Damaged {
                    range: 0,
                    owner: 0,
                    start: 1,
                    len: 3,
                    captains: 0b01
                },
                Walk::Unevaluated {
                    owner: 0,
                    start: 4,
                    len: 2
                },
                Walk::Unevaluated {
                    owner: 1,
                    start: 4,
                    len: 2
                },
            ]
        );
    }

    /// A range of `group` at `start`, `len` long with `shift` and `trust`.
    fn group_range(
        group: &Arc<[StorageId]>,
        start: usize,
        len: usize,
        shift: isize,
        trust: usize,
    ) -> GroupRange {
        GroupRange {
            group: group.clone(),
            start,
            len,
            shift,
            trust,
        }
    }

    /// Group metadata of storage 0 with `ranges` and the unevaluated region at `unevaluated_start`.
    fn storage_groups(ranges: Vec<GroupRange>, unevaluated_start: usize) -> Groups {
        Groups {
            storage_id: Some(StorageId::Custom(0)),
            ranges,
            unevaluated_start,
        }
    }

    /// Returns the window metadata of a storage of `len` components with `groups`.
    fn metadata(groups: &Groups, len: usize) -> Option<WindowMetadata> {
        match NodeMetadata::of_storage(groups, len) {
            NodeMetadata::Window(window) => Some(window),
            _ => None,
        }
    }

    /// Asserts that a storage without group reports no metadata.
    #[test]
    fn metadata_ungrouped() {
        assert!(metadata(&Groups::default(), 5).is_none());
    }

    /// Asserts that a storage whose metadata doesn't record its own id reports nothing.
    #[test]
    fn metadata_missing_id() {
        let mut groups = storage_groups(vec![group_range(&group(), 0, 2, 0, 2)], 2);
        groups.storage_id = None;

        assert!(metadata(&groups, 2).is_none());
    }

    /// Asserts that every range is reported with its group, position, rotation and trust,
    /// empty ones included, along with the storage-wide regions.
    #[test]
    fn metadata_ranges() {
        let ab = group();
        let ac: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(2)]);
        let ad: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(3)]);
        let groups = storage_groups(
            vec![
                group_range(&ab, 0, 2, 1, 2),
                group_range(&ac, 2, 0, -3, 0),
                group_range(&ad, 2, 3, 4, 1),
            ],
            6,
        );

        let window = metadata(&groups, 8).unwrap();

        assert_eq!(window.storage_id, StorageId::Custom(0));
        assert_eq!((window.len, window.unevaluated_start), (8, 6));
        let rings: Vec<(Ring, usize)> = window
            .ranges
            .iter()
            .map(|range| (range.ring, range.trust))
            .collect();
        assert_eq!(
            rings,
            [
                (
                    Ring {
                        start: 0,
                        len: 2,
                        shift: 1
                    },
                    2
                ),
                (
                    Ring {
                        start: 2,
                        len: 0,
                        shift: -3
                    },
                    0
                ),
                (
                    Ring {
                        start: 2,
                        len: 3,
                        shift: 4
                    },
                    1
                ),
            ]
        );
        for (range, group) in window.ranges.iter().zip([&ab, &ac, &ad]) {
            assert!(Arc::ptr_eq(&range.group, group));
        }
    }

    /// Asserts that a reset storage reports its emptied ranges, they're still a valid layout.
    #[test]
    fn metadata_reset() {
        let ac: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(2)]);
        let mut groups = storage_groups(
            vec![
                group_range(&group(), 0, 2, 0, 2),
                group_range(&ac, 2, 3, -1, 3),
            ],
            6,
        );

        groups.reset();
        let window = metadata(&groups, 6).unwrap();

        assert_eq!(window.unevaluated_start, 0);
        assert!(window
            .ranges
            .iter()
            .all(|range| range.ring.start == 0 && range.ring.len == 0 && range.trust == 0));
    }

    /// Asserts that layouts outside the storage or out of order aren't trusted.
    #[test]
    fn metadata_invalid() {
        let ab = group();
        let ac: Arc<[StorageId]> = Arc::from([StorageId::Custom(0), StorageId::Custom(2)]);
        let invalid = [
            // The first range doesn't start the storage.
            (vec![group_range(&ab, 1, 2, 0, 2)], 3, 3),
            // A gap between two ranges.
            (
                vec![group_range(&ab, 0, 2, 0, 2), group_range(&ac, 3, 1, 0, 1)],
                4,
                4,
            ),
            // Trust past the range's length.
            (vec![group_range(&ab, 0, 2, 0, 3)], 2, 2),
            // The ranges overlap the unevaluated region.
            (vec![group_range(&ab, 0, 3, 0, 3)], 2, 3),
            // The unevaluated region starts past the storage's end.
            (vec![group_range(&ab, 0, 2, 0, 2)], 5, 4),
            // The ranges' end overflows.
            (
                vec![
                    group_range(&ab, 0, usize::MAX, 0, 0),
                    group_range(&ac, usize::MAX, 2, 0, 0),
                ],
                usize::MAX,
                usize::MAX,
            ),
        ];

        for (ranges, unevaluated_start, len) in invalid {
            let groups = storage_groups(ranges, unevaluated_start);

            assert!(metadata(&groups, len).is_none());
        }
    }

    /// Asserts that a child unable to sail drives and disables grouping.
    #[test]
    fn forced_captain() {
        let group = group();
        let mut entities = ungrouped(100);
        entities.can_sailor = false;
        let mut children = [member(&group, 0, 4, 4), member(&group, 1, 4, 4), entities];

        let selection = select(&mut children);

        assert_eq!(selection.ordinary.captain, 2);
        assert!(selection.alternatives.is_empty());
    }

    /// A nested `Mixed` child of `lookups` windows whose ordinary walk drives `candidates`
    /// candidates for `work`, reporting `alternatives` as `(candidates, work)`.
    fn nested(
        candidates: usize,
        work: u64,
        lookups: usize,
        alternatives: &[(usize, u64)],
    ) -> Child {
        let alternatives = alternatives
            .iter()
            .map(|&(candidates, work)| Alternative {
                candidates,
                work,
                route: Route::Local(GroupedCandidate {
                    required: 0,
                    ranges: Vec::new(),
                    members: Vec::new(),
                    walks: Vec::new(),
                }),
            })
            .collect();

        Child {
            can_captain: true,
            can_sailor: true,
            sail_time: candidates,
            candidates: Some(candidates),
            entities: EntityCursor::empty(),
            metadata: NodeMetadata::Mixed(Selection {
                ordinary: OrdinaryCandidate {
                    captain: 0,
                    work: Some(work),
                },
                alternatives,
                lookups,
                filtered: false,
            }),
        }
    }

    /// Asserts that a nested `Mixed` as sailor costs one lookup per window below it,
    /// and that its parent reports the sum of its children's lookups.
    #[test]
    fn nested_lookups() {
        let mut children = [ungrouped(4), nested(100, 300, 3, &[])];

        let selection = select(&mut children);

        assert_eq!(selection.lookups, 4);
        assert_eq!(selection.ordinary.captain, 0);
        // 4 candidates looking 3 windows up.
        assert_eq!(selection.ordinary.work, Some(4 * (2 + 3)));
    }

    /// Asserts that a nested `Mixed` driving the ordinary walk brings its own work,
    /// its parent only adds the lookups of its other children.
    #[test]
    fn nested_captain() {
        let mut children = [nested(4, 30, 2, &[]), ungrouped(100)];

        let selection = select(&mut children);

        assert_eq!(selection.ordinary.captain, 0);
        assert_eq!(selection.ordinary.work, Some(30 + 4));
        assert!(selection.alternatives.is_empty());
    }

    /// Asserts that a parent ranks every alternative a child reported with its own lookups:
    /// the child's cheaper walk has more candidates and loses once the parent looks 3 windows up
    /// for each of them. The child isn't committed to its own cheapest walk.
    #[test]
    fn parent_reranks() {
        let mut children = [
            nested(1000, 5000, 2, &[(50, 100), (10, 110)]),
            ungrouped(1000),
            ungrouped(1000),
            ungrouped(1000),
        ];

        let selection = select(&mut children);

        let works: Vec<u64> = selection
            .alternatives
            .iter()
            .map(|alternative| alternative.work)
            .collect();
        assert_eq!(works, [100 + 50 * 3, 110 + 10 * 3]);

        let winner = selection.winner().unwrap();
        let Route::Nested { child, alternative } = &winner.route else {
            panic!("the child's alternative drives");
        };
        assert_eq!(
            (*child, alternative.candidates, alternative.work),
            (0, 10, 110)
        );
        assert_eq!(winner.candidates, 10);
    }

    /// Asserts that alternatives move up through every level, each one adding its lookups.
    #[test]
    fn two_levels() {
        let mut middle_children = [nested(20, 60, 2, &[(10, 20)]), ungrouped(30)];
        let middle = select(&mut middle_children);
        assert_eq!(middle.lookups, 3);
        assert_eq!(middle.alternatives[0].work, 20 + 10);

        let mut root_children = [
            Child {
                metadata: NodeMetadata::Mixed(middle),
                ..ungrouped(20)
            },
            ungrouped(40),
        ];
        let root = select(&mut root_children);

        let winner = root.winner().unwrap();
        assert_eq!(winner.work, 20 + 10 + 10);
        let Route::Nested {
            child: 0,
            alternative,
        } = &winner.route
        else {
            panic!("the middle `Mixed` drives");
        };
        assert!(matches!(alternative.route, Route::Nested { child: 0, .. }));
    }

    /// Asserts that a filtered child drops every alternative, its siblings' included,
    /// and that a child unable to sail drops nested alternatives too.
    #[test]
    fn nested_disabled() {
        let mut filtered = ungrouped(5);
        filtered.metadata = NodeMetadata::Filtered;
        let mut children = [nested(10, 30, 2, &[(10, 20)]), filtered];
        assert!(select(&mut children).alternatives.is_empty());

        let mut entities = ungrouped(100);
        entities.can_sailor = false;
        let mut children = [nested(10, 30, 2, &[(10, 20)]), entities];
        assert!(select(&mut children).alternatives.is_empty());
    }

    /// Asserts that a nested alternative whose estimate overflows with its parent's lookups
    /// is left out instead of wrapping.
    #[test]
    fn nested_overflow() {
        let mut children = [
            nested(10, 30, 2, &[(usize::MAX, u64::MAX - 1)]),
            ungrouped(1),
        ];

        assert!(select(&mut children).alternatives.is_empty());
    }
}

#[cfg(test)]
mod grouping_cover_sets {
    use super::*;

    const A: u64 = 0;
    const B: u64 = 1;
    const C: u64 = 2;
    const D: u64 = 3;

    /// Returns the group made of the storages `ids`.
    fn group(ids: &[u64]) -> Arc<[StorageId]> {
        let mut ids: Vec<StorageId> = ids.iter().map(|&id| StorageId::Custom(id)).collect();
        ids.sort_unstable();

        Arc::from(ids)
    }

    /// A window child over storage `storage` holding a range for each of `groups`,
    /// back to back, each with 2 fully trusted components.
    fn window(storage: u64, groups: &[&Arc<[StorageId]>]) -> Child {
        let ranges: Vec<RangeMetadata> = groups
            .iter()
            .enumerate()
            .map(|(position, &group)| RangeMetadata {
                group: group.clone(),
                ring: Ring {
                    start: position * 2,
                    len: 2,
                    shift: 0,
                },
                trust: 2,
            })
            .collect();
        let len = ranges.len() * 2;

        Child {
            can_captain: true,
            can_sailor: true,
            sail_time: len,
            candidates: Some(len),
            entities: EntityCursor::empty(),
            metadata: NodeMetadata::Window(WindowMetadata {
                storage_id: StorageId::Custom(storage),
                len,
                unevaluated_start: len,
                ranges,
            }),
        }
    }

    /// A nested `Mixed` child, it never holds group members of its parent.
    fn nested() -> Child {
        Child {
            metadata: NodeMetadata::Mixed(Selection {
                ordinary: OrdinaryCandidate {
                    captain: 0,
                    work: Some(4),
                },
                alternatives: Vec::new(),
                lookups: 1,
                filtered: false,
            }),
            ..window(9, &[])
        }
    }

    /// Returns the groups of `set`'s ranges, in order.
    fn range_groups(set: &CoverSet) -> Vec<Arc<[StorageId]>> {
        set.ranges.iter().map(|range| range.group.clone()).collect()
    }

    /// Returns the children taking part in `range`, in order.
    fn range_children(range: &CoverRange) -> Vec<usize> {
        range.members.iter().map(|member| member.child).collect()
    }

    /// Asserts that with AB and AC and no ABC union yet, an `(A, B, C)` query has to cover
    /// A, B and C, with no range to walk: its matches can only be unevaluated.
    #[test]
    fn absent_union() {
        let (ab, ac) = (group(&[A, B]), group(&[A, C]));
        let children = [window(A, &[&ab, &ac]), window(B, &[&ab]), window(C, &[&ac])];

        let sets = cover_sets(&children);

        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].required, 0b111);
        assert!(sets[0].ranges.is_empty());
    }

    /// Asserts that with AB, AC and their ABC union, an `(A, B, C)` query only walks ABC,
    /// AB and AC can't hold a fully evaluated match.
    #[test]
    fn union_only() {
        let (ab, ac, abc) = (group(&[A, B]), group(&[A, C]), group(&[A, B, C]));
        let children = [
            window(A, &[&ab, &ac, &abc]),
            window(B, &[&ab, &abc]),
            window(C, &[&ac, &abc]),
        ];

        let sets = cover_sets(&children);

        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].required, 0b111);
        assert_eq!(sets[0].ranges.len(), 1);
        assert!(Arc::ptr_eq(&range_groups(&sets[0])[0], &abc));
        assert_eq!(range_children(&sets[0].ranges[0]), [0, 1, 2]);
    }

    /// Asserts that with AB, AC and ABC, an `(A, B)` query walks AB and ABC
    /// using A and B alone, C isn't borrowed.
    #[test]
    fn projection() {
        let (ab, ac, abc) = (group(&[A, B]), group(&[A, C]), group(&[A, B, C]));
        let children = [window(A, &[&ab, &ac, &abc]), window(B, &[&ab, &abc])];

        let sets = cover_sets(&children);

        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].required, 0b11);
        let groups = range_groups(&sets[0]);
        assert_eq!(groups.len(), 2);
        assert!(Arc::ptr_eq(&groups[0], &ab) && Arc::ptr_eq(&groups[1], &abc));
        assert_eq!(range_children(&sets[0].ranges[1]), [0, 1]);
    }

    /// Asserts that an ABC group alone doesn't cover an `(A, B)` query,
    /// AB matches without C sit outside its ranges.
    #[test]
    fn larger_only() {
        let abc = group(&[A, B, C]);
        let children = [window(A, &[&abc]), window(B, &[&abc])];

        assert!(cover_sets(&children).is_empty());
    }

    /// Asserts that disjoint AB and CD give distinct sets, ordered by their first child.
    #[test]
    fn disjoint() {
        let (ab, cd) = (group(&[A, B]), group(&[C, D]));
        let children = [
            window(C, &[&cd]),
            window(D, &[&cd]),
            window(A, &[&ab]),
            window(B, &[&ab]),
        ];

        let sets = cover_sets(&children);

        let required: Vec<u32> = sets.iter().map(|set| set.required).collect();
        assert_eq!(required, [0b0011, 0b1100]);
        assert!(Arc::ptr_eq(&range_groups(&sets[0])[0], &cd));
        assert!(Arc::ptr_eq(&range_groups(&sets[1])[0], &ab));
    }

    /// Asserts that with AB and ACD, an `(A, B, C)` query covers AB, walking AB and the ABCD
    /// union, where C comes along as an extra member and D isn't borrowed.
    #[test]
    fn extra_member() {
        let (ab, acd, abcd) = (group(&[A, B]), group(&[A, C, D]), group(&[A, B, C, D]));
        let children = [
            window(A, &[&ab, &acd, &abcd]),
            window(B, &[&ab, &abcd]),
            window(C, &[&acd, &abcd]),
        ];

        let sets = cover_sets(&children);

        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].required, 0b011);
        let groups = range_groups(&sets[0]);
        assert_eq!(groups.len(), 2);
        assert!(Arc::ptr_eq(&groups[0], &ab) && Arc::ptr_eq(&groups[1], &abcd));
        assert_eq!(range_children(&sets[0].ranges[0]), [0, 1]);
        assert_eq!(range_children(&sets[0].ranges[1]), [0, 1, 2]);
    }

    /// Asserts that overlaps are transitive: AB, BC and CD give a single set.
    #[test]
    fn transitive() {
        let (ab, bc, cd) = (group(&[A, B]), group(&[B, C]), group(&[C, D]));
        let children = [
            window(A, &[&ab]),
            window(B, &[&ab, &bc]),
            window(C, &[&bc, &cd]),
            window(D, &[&cd]),
        ];

        let sets = cover_sets(&children);

        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].required, 0b1111);
        assert!(sets[0].ranges.is_empty());
    }

    /// Asserts that a declared group and a larger one containing it are joined,
    /// only the larger one's range contains every required member.
    #[test]
    fn subset() {
        let (ab, abc) = (group(&[A, B]), group(&[A, B, C]));
        let children = [
            window(A, &[&ab, &abc]),
            window(B, &[&ab, &abc]),
            window(C, &[&abc]),
        ];

        let sets = cover_sets(&children);

        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].required, 0b111);
        assert_eq!(sets[0].ranges.len(), 1);
        assert!(Arc::ptr_eq(&range_groups(&sets[0])[0], &abc));
    }

    /// Asserts that a group with a member held by two children isn't used,
    /// we don't pick which one would drive.
    #[test]
    fn ambiguous_required() {
        let ab = group(&[A, B]);
        let children = [window(A, &[&ab]), window(A, &[&ab]), window(B, &[&ab])];

        assert!(cover_sets(&children).is_empty());
    }

    /// Asserts that an extra member held by two children doesn't come along.
    #[test]
    fn ambiguous_extra() {
        let (ab, acd, abcd) = (group(&[A, B]), group(&[A, C, D]), group(&[A, B, C, D]));
        let children = [
            window(A, &[&ab, &acd, &abcd]),
            window(B, &[&ab, &abcd]),
            window(C, &[&acd, &abcd]),
            window(C, &[&acd, &abcd]),
        ];

        let sets = cover_sets(&children);

        assert_eq!(sets.len(), 1);
        assert_eq!(range_children(&sets[0].ranges[1]), [0, 1]);
    }

    /// Asserts that groups don't cross `Mixed` boundaries: a member held below a nested `Mixed`
    /// doesn't make its parent hold the group.
    #[test]
    fn nested_boundary() {
        let ab = group(&[A, B]);
        let children = [window(A, &[&ab]), nested()];

        assert!(cover_sets(&children).is_empty());
    }

    /// Asserts that each member reports its own part of a range, in child order,
    /// whatever the order of the group's ids.
    #[test]
    fn member_parts() {
        let ab = group(&[A, B]);
        let mut a = window(A, &[&ab]);
        if let NodeMetadata::Window(window) = &mut a.metadata {
            window.ranges[0].ring.shift = -1;
            window.ranges[0].trust = 1;
        }
        let children = [window(B, &[&ab]), a];

        let sets = cover_sets(&children);

        assert_eq!(sets[0].required, 0b11);
        assert_eq!(
            sets[0].ranges[0].members,
            [
                RangeMember {
                    child: 0,
                    ring: Ring {
                        start: 0,
                        len: 2,
                        shift: 0,
                    },
                    trust: 2,
                },
                RangeMember {
                    child: 1,
                    ring: Ring {
                        start: 0,
                        len: 2,
                        shift: -1,
                    },
                    trust: 1,
                },
            ]
        );
    }

    /// A window child over storage `storage` holding a range for each `(group, len, trust)`,
    /// back to back and unrotated, followed by `unevaluated` unevaluated components.
    fn window_with(
        storage: u64,
        ranges: &[(&Arc<[StorageId]>, usize, usize)],
        unevaluated: usize,
    ) -> Child {
        let mut child = window(storage, &[]);
        let NodeMetadata::Window(window) = &mut child.metadata else {
            unreachable!();
        };

        for &(group, len, trust) in ranges {
            window.ranges.push(RangeMetadata {
                group: group.clone(),
                ring: Ring {
                    start: window.len,
                    len,
                    shift: 0,
                },
                trust,
            });
            window.len += len;
        }
        window.unevaluated_start = window.len;
        window.len += unevaluated;
        child.candidates = Some(window.len);
        child.sail_time = window.len;

        child
    }

    /// Returns the walks of the first cover set of `children`, placeable or not.
    fn first_candidate(children: &[Child]) -> GroupedCandidate {
        let set = cover_sets(children)
            .into_iter()
            .next()
            .expect("the children hold a group");

        GroupedCandidate::new(children, set)
    }

    fn aligned(range: usize, start: usize, len: usize, captains: u32) -> Walk {
        Walk::Aligned {
            range,
            start,
            len,
            captains,
        }
    }

    fn damaged(range: usize, owner: usize, start: usize, len: usize, captains: u32) -> Walk {
        Walk::Damaged {
            range,
            owner,
            start,
            len,
            captains,
        }
    }

    fn unevaluated(owner: usize, start: usize, len: usize) -> Walk {
        Walk::Unevaluated { owner, start, len }
    }

    /// Asserts that an `(A, B)` query walks the aligned part of each range containing A and B,
    /// AB then the ABC union, where A and B read directly and C isn't involved.
    #[test]
    fn walks_ranges() {
        let (ab, ac, abc) = (group(&[A, B]), group(&[A, C]), group(&[A, B, C]));
        let children = [
            window_with(A, &[(&ab, 2, 2), (&ac, 1, 1), (&abc, 3, 3)], 0),
            window_with(B, &[(&ab, 2, 2), (&abc, 3, 3)], 0),
        ];

        let candidate = first_candidate(&children);

        assert_eq!(
            candidate.walks,
            [aligned(0, 0, 2, 0b11), aligned(1, 0, 3, 0b11)]
        );
        // 5 candidates without sailor.
        let alternative = Alternative::local(candidate, 2).unwrap();
        assert_eq!((alternative.candidates, alternative.work), (5, 10));
    }

    /// Asserts that unevaluated walks come once, after every range walk,
    /// longest first and in child order on ties.
    #[test]
    fn walks_unevaluated_once() {
        let (ab, abc) = (group(&[A, B]), group(&[A, B, C]));
        let children = [
            window_with(A, &[(&ab, 2, 2), (&abc, 1, 1)], 2),
            window_with(B, &[(&ab, 2, 2), (&abc, 1, 1)], 3),
            window_with(D, &[], 0),
        ];

        let candidate = first_candidate(&children);

        assert_eq!(
            candidate.walks,
            [
                aligned(0, 0, 2, 0b11),
                aligned(1, 0, 1, 0b11),
                unevaluated(1, 3, 3),
                unevaluated(0, 3, 2),
            ]
        );
    }

    /// Asserts that the aligned prefix stops at the lowest trust of the required members,
    /// the shortest damaged suffix drives the rest, and a member trusting positions the owner
    /// doesn't trust can't read directly there.
    #[test]
    fn walks_required_trust() {
        let ab = group(&[A, B]);
        let children = [
            window_with(A, &[(&ab, 5, 1)], 0),
            window_with(B, &[(&ab, 4, 3)], 0),
        ];

        let candidate = first_candidate(&children);

        // B's suffix is the shortest, A trusts none of it.
        assert_eq!(
            candidate.walks,
            [aligned(0, 0, 1, 0b11), damaged(0, 1, 1, 3, 0b10)]
        );
    }

    /// Asserts that on equal damaged suffixes, the first required member in child order owns
    /// the walk, and B trusting more than the owner doesn't make it a captain.
    #[test]
    fn walks_damaged_tie() {
        let ab = group(&[A, B]);
        let children = [
            window_with(A, &[(&ab, 4, 1)], 0),
            window_with(B, &[(&ab, 4, 3)], 0),
        ];

        assert_eq!(
            first_candidate(&children).walks,
            [aligned(0, 0, 1, 0b11), damaged(0, 0, 1, 3, 0b01)]
        );
    }

    /// Returns `(A, B, C)` children with AB, ACD and the ABCD union, D isn't borrowed.
    /// C trusts `c_trust` of ABCD's 6 positions, A and B trust all of them.
    fn extra_children(c_trust: usize) -> [Child; 3] {
        let (ab, acd, abcd) = (group(&[A, B]), group(&[A, C, D]), group(&[A, B, C, D]));

        [
            window_with(A, &[(&ab, 2, 2), (&acd, 1, 1), (&abcd, 6, 6)], 0),
            window_with(B, &[(&ab, 2, 2), (&abcd, 6, 6)], 0),
            window_with(C, &[(&acd, 1, 1), (&abcd, 6, c_trust)], 0),
        ]
    }

    /// Asserts that an extra member trusting none of a range is looked up all along,
    /// A and B still read directly over the whole range.
    #[test]
    fn walks_extra_untrusted() {
        assert_eq!(
            first_candidate(&extra_children(0)).walks,
            [aligned(0, 0, 2, 0b011), aligned(1, 0, 6, 0b011)]
        );
    }

    /// Asserts that an extra member reads directly up to its trust, splitting the aligned
    /// part there, without shortening A and B's aligned part.
    #[test]
    fn walks_extra_partial() {
        let candidate = first_candidate(&extra_children(3));

        assert_eq!(
            candidate.walks,
            [
                aligned(0, 0, 2, 0b011),
                aligned(1, 0, 3, 0b111),
                aligned(1, 3, 3, 0b011),
            ]
        );
        // AB's 2 and ABCD's last 3 candidates look C up, ABCD's first 3 look nothing up.
        let alternative = Alternative::local(candidate, 3).unwrap();
        assert_eq!(
            (alternative.candidates, alternative.work),
            (8, 2 * 3 + 3 * 2 + 3 * 3)
        );
    }

    /// Asserts that an extra member trusting a whole range reads directly over all of it.
    #[test]
    fn walks_extra_trusted() {
        assert_eq!(
            first_candidate(&extra_children(6)).walks,
            [aligned(0, 0, 2, 0b011), aligned(1, 0, 6, 0b111)]
        );
    }

    /// Asserts that a damaged suffix is split at every trust endpoint: members read directly
    /// where they and the owner both trust the position, then the owner drives alone.
    #[test]
    fn walks_damaged_extras() {
        let (ab, acd, abcd) = (group(&[A, B]), group(&[A, C, D]), group(&[A, B, C, D]));
        let children = [
            window_with(A, &[(&ab, 0, 0), (&acd, 0, 0), (&abcd, 8, 2)], 0),
            window_with(B, &[(&ab, 0, 0), (&abcd, 6, 5)], 0),
            window_with(C, &[(&acd, 0, 0), (&abcd, 7, 4)], 0),
        ];

        let candidate = first_candidate(&children);

        // B has the shortest suffix. C trusts up to 4 and B up to 5, past that B drives alone.
        assert_eq!(
            candidate.walks,
            [
                aligned(1, 0, 2, 0b111),
                damaged(1, 1, 2, 2, 0b110),
                damaged(1, 1, 4, 2, 0b010),
            ]
        );
    }

    /// Asserts that a cover set without any containing range, AB and AC before their ABC union
    /// exists, only walks its required members' unevaluated regions.
    #[test]
    fn walks_unevaluated_only() {
        let (ab, ac) = (group(&[A, B]), group(&[A, C]));
        let children = [
            window_with(A, &[(&ab, 0, 0), (&ac, 0, 0)], 3),
            window_with(B, &[(&ab, 0, 0)], 1),
            window_with(C, &[(&ac, 0, 0)], 2),
        ];

        let candidate = first_candidate(&children);

        assert_eq!(
            candidate.walks,
            [
                unevaluated(0, 0, 3),
                unevaluated(2, 0, 2),
                unevaluated(1, 0, 1)
            ]
        );
        // Every candidate looks the two other children up.
        let alternative = Alternative::local(candidate, 3).unwrap();
        assert_eq!((alternative.candidates, alternative.work), (6, 6 * 4));
    }

    /// Asserts that a grouped walk over several ranges can cost more than an ordinary walk
    /// with a small captain.
    #[test]
    fn estimates_ordinary() {
        let (ab, abc) = (group(&[A, B]), group(&[A, B, C]));
        let children = [
            window_with(A, &[(&ab, 20, 20), (&abc, 30, 30)], 0),
            window_with(B, &[(&ab, 20, 20), (&abc, 30, 30)], 0),
            window_with(9, &[], 3),
        ];

        let grouped = Alternative::local(first_candidate(&children), 3).unwrap();
        let ordinary = ordinary_candidate(&children, 3);

        // 50 candidates looking the small storage up against 3 candidates looking A and B up.
        assert_eq!(grouped.work, 50 * 3);
        assert_eq!((ordinary.captain, ordinary.work), (2, Some(3 * 4)));
    }

    /// Asserts that disjoint cover sets get their own walks and estimates,
    /// the smaller group's being cheaper.
    #[test]
    fn estimates_disjoint() {
        let (ab, cd) = (group(&[A, B]), group(&[C, D]));
        let children = [
            window_with(A, &[(&ab, 40, 40)], 0),
            window_with(B, &[(&ab, 40, 40)], 0),
            window_with(C, &[(&cd, 10, 10)], 0),
            window_with(D, &[(&cd, 10, 10)], 0),
        ];

        let works: Vec<u64> = cover_sets(&children)
            .into_iter()
            .map(|set| {
                Alternative::local(GroupedCandidate::new(&children, set), 4)
                    .unwrap()
                    .work
            })
            .collect();

        // Each walk reads its two members and looks the two others up.
        assert_eq!(works, [40 * 4, 10 * 4]);
    }

    /// Asserts that `pieces` splits only where the captains change, ignores bounds outside
    /// the positions and gives nothing for empty positions.
    #[test]
    fn split_pieces() {
        let captains_at = |position: usize| if position < 4 { 0b11 } else { 0b01 };

        assert_eq!(
            pieces(1..7, [2, 4, 4, 9, 0].into_iter(), captains_at),
            [(1..4, 0b11), (4..7, 0b01)]
        );
        assert!(pieces(3..3, [3].into_iter(), captains_at).is_empty());
    }
}
