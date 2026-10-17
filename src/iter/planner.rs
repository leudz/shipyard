//! Construction-only choice between an ordinary walk and walks over a group.
//!
//! While a query is built, each node reports what it knows about grouping in a
//! [`NodeMetadata`], read from its storages while they're borrowed.\
//! A `Mixed` hands its immediate children to [`select`], which ranks the ordinary captains
//! and estimates its grouped [`Alternative`]s: walks over every group its window children
//! fully hold, and the alternatives its nested `Mixed` children reported, driven through them.\
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
use crate::sparse_set::{Groups, SparseSet};
use crate::storage::StorageId;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Reverse;
use core::fmt;

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
    /// A plain window over a storage holding a group range.
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
        // A storage can only hold a single range for now. With several, picking one would
        // need union support, so we don't advertise any and stay ordinary, which is always complete.
        let [range] = &groups.ranges[..] else {
            return NodeMetadata::Ungrouped;
        };
        let Some(storage_id) = groups.storage_id else {
            return NodeMetadata::Ungrouped;
        };

        let regions = MemberRegions {
            range_start: range.start,
            range_len: range.len,
            trust: range.trust,
            unevaluated_start: groups.unevaluated_start,
            len,
        };

        // Rotated ranges can't be walked yet and the estimates subtract these bounds,
        // so anything outside the base's layout stays ordinary instead of being trusted.
        if range.shift != 0 || !regions.is_valid() {
            return NodeMetadata::Ungrouped;
        }

        NodeMetadata::Window(WindowMetadata {
            storage_id,
            group: range.group.clone(),
            regions,
        })
    }
}

/// Group metadata of a plain window's storage.
#[derive(Clone)]
pub(crate) struct WindowMetadata {
    /// Id of the storage, as its own group metadata records it.
    ///
    /// Thread-local storages have ids distinct from their `Send + Sync` counterparts.
    pub(crate) storage_id: StorageId,
    /// Sorted member ids of the storage's group, compared by pointer.
    pub(crate) group: Arc<[StorageId]>,
    pub(crate) regions: MemberRegions,
}

/// Regions of a group member's storage.
///
/// ```text
/// [ Group range ][ Ungrouped ][ Unevaluated ]
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct MemberRegions {
    /// Physical index of the group range's first component.
    pub(crate) range_start: usize,
    pub(crate) range_len: usize,
    /// Length of the range's prefix unchanged since the last regroup.
    pub(crate) trust: usize,
    /// Physical index where the unevaluated region starts.
    pub(crate) unevaluated_start: usize,
    /// Number of components in the storage.
    pub(crate) len: usize,
}

impl MemberRegions {
    /// Returns `true` when the regions are in order and within the storage.
    fn is_valid(&self) -> bool {
        let range_end = self.range_start.checked_add(self.range_len);

        self.trust <= self.range_len
            && range_end.is_some_and(|range_end| range_end <= self.unevaluated_start)
            && self.unevaluated_start <= self.len
    }

    /// Returns the number of components in the unevaluated region.
    fn unevaluated_len(&self) -> usize {
        self.len - self.unevaluated_start
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

/// Walks covering every match of a query through one of its groups.
///
/// This is the region recipe: the walks are listed in the order they're emitted
/// and the estimate is computed from these same walks.
#[derive(Clone, Debug)]
pub(crate) struct GroupedCandidate {
    /// The group's members in child order, with their storage's regions.
    pub(crate) members: Vec<Member>,
    /// Walks in emission order, empty ones are left out.
    pub(crate) walks: Vec<Walk>,
}

/// A group member and the child holding it.
#[derive(Clone, Copy)]
pub(crate) struct Member {
    /// Position of the child among the `Mixed`'s children.
    pub(crate) child: usize,
    pub(crate) regions: MemberRegions,
    /// Every entity of the member's storage, `regions` describes them.
    pub(crate) entities: EntityCursor,
}

impl fmt::Debug for Member {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Member")
            .field("child", &self.child)
            .field("regions", &self.regions)
            .finish_non_exhaustive()
    }
}

/// One walk of a grouped alternative.
///
/// Range positions are logical, counted from the start of each member's range.
/// Turning them into physical indices is left to whoever emits the walks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Walk {
    /// Every member drives the first `len` positions of its range, where all members hold
    /// the same entities. Other children are looked up.
    Aligned { len: usize },
    /// The child at position `owner` drives positions `start..start + len` of its range.
    /// All other children are looked up.
    Damaged {
        owner: usize,
        start: usize,
        len: usize,
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
            Walk::Aligned { len } | Walk::Damaged { len, .. } | Walk::Unevaluated { len, .. } => {
                len
            }
        }
    }

    /// Returns the number of children driving the walk, out of a group of `member_count`.
    fn captain_count(&self, member_count: usize) -> usize {
        match self {
            Walk::Aligned { .. } => member_count,
            Walk::Damaged { .. } | Walk::Unevaluated { .. } => 1,
        }
    }
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
/// Alternatives are ordered by the first child they involve: a group by its first member,
/// a nested `Mixed`'s alternatives by its position.\
/// Alternatives whose estimate doesn't fit in a `u64` can't beat anything and are left out.
fn alternatives(children: &mut [Child], lookups: usize) -> Vec<Alternative> {
    let mut alternatives = Vec::new();

    for position in 0..children.len() {
        // Only a window can start a group and only a nested `Mixed` has alternatives,
        // at most one of these two adds something.
        if let Some(candidate) = local_candidate(children, position) {
            alternatives.extend(Alternative::local(candidate, lookups));
        }

        // Groups don't cross `Mixed` boundaries: a nested `Mixed` is never a member of our
        // groups and we drive through its alternatives as a whole, its masks stay its own.
        let child = &mut children[position];
        let sailor_lookups = lookups - child.lookups();
        for alternative in child.take_alternatives() {
            alternatives.extend(Alternative::nested(position, alternative, sailor_lookups));
        }
    }

    alternatives
}

/// Returns the walks over the group whose first member is the child at `position`,
/// `None` when that child isn't a window over a grouped storage or `children` don't fully
/// hold its group.
fn local_candidate(children: &[Child], position: usize) -> Option<GroupedCandidate> {
    let window = children[position].window()?;

    // Each group is considered once, from its first member.
    let seen = children[..position]
        .iter()
        .filter_map(Child::window)
        .any(|earlier| Arc::ptr_eq(&earlier.group, &window.group));
    if seen {
        return None;
    }

    let members = group_members(children, &window.group)?;

    Some(GroupedCandidate::new(members))
}

/// Returns `group`'s members, in child order, when each of them is held by exactly one child.
///
/// A missing member makes the group unusable: with an `(A, B)` query and only an ABC group,
/// matches without C sit outside the group range and only an ordinary walk finds them.\
/// A member held by several children, like A in `(&a, &a, &b)`, is ambiguous and we don't
/// pick which child would drive.
fn group_members(children: &[Child], group: &Arc<[StorageId]>) -> Option<Vec<Member>> {
    let mut members = Vec::new();
    // Bit `i` flags the `i`th member id as found.
    let mut found = 0u64;

    for (position, child) in children.iter().enumerate() {
        let Some(window) = child.window() else {
            continue;
        };
        if !Arc::ptr_eq(&window.group, group) {
            continue;
        }

        // Group ids are sorted, a member's position in them identifies it.
        let member_index = group.binary_search(&window.storage_id).ok()?;
        let bit = 1u64.checked_shl(u32::try_from(member_index).ok()?)?;
        if found & bit != 0 {
            return None;
        }

        found |= bit;
        members.push(Member {
            child: position,
            regions: window.regions,
            entities: child.entities,
        });
    }

    (members.len() == group.len()).then_some(members)
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
            let sailor_lookups = lookups - walk.captain_count(candidate.members.len());

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
    #[allow(dead_code, reason = "Only tests read it for now.")]
    pub(crate) fn group(&self) -> &GroupedCandidate {
        match &self.route {
            Route::Local(candidate) => candidate,
            Route::Nested { alternative, .. } => alternative.group(),
        }
    }
}

impl GroupedCandidate {
    /// Returns the walks over the group of `members`.
    fn new(members: Vec<Member>) -> GroupedCandidate {
        let walks = walks(&members);

        GroupedCandidate { members, walks }
    }

    /// Returns the children holding the group's members, bit `i` for child `i`.
    #[allow(dead_code, reason = "Only tests read it for now.")]
    pub(crate) fn member_mask(&self) -> u32 {
        self.members
            .iter()
            .fold(0, |mask, member| mask | (1 << member.child))
    }
}

/// Returns the walks over the group of `members`, in emission order.
///
/// Empty walks are left out.
fn walks(members: &[Member]) -> Vec<Walk> {
    let mut walks = Vec::new();

    // Every member holds the same entities over the prefix all of them trust.
    let aligned_len = members
        .iter()
        .map(|member| member.regions.trust)
        .min()
        .expect("groups have at least two members");

    if aligned_len > 0 {
        walks.push(Walk::Aligned { len: aligned_len });
    }

    // A match whose members are all in the group range but outside the aligned prefix is
    // in every member's damaged suffix. Walking a single one of them is enough,
    // we pick the shortest, the first in child order on ties.
    let damaged_owner = members
        .iter()
        .min_by_key(|member| member.regions.range_len - aligned_len)
        .expect("groups have at least two members");
    let damaged_len = damaged_owner.regions.range_len - aligned_len;

    if damaged_len > 0 {
        walks.push(Walk::Damaged {
            owner: damaged_owner.child,
            start: aligned_len,
            len: damaged_len,
        });
    }

    // Each member walks its unevaluated region. A walk rejects what the walks before it
    // covered, so later walks check more regions: the longest go first, in child order on ties.
    let mut unevaluated: Vec<&Member> = members.iter().collect();
    unevaluated.sort_by_key(|member| Reverse(member.regions.unevaluated_len()));

    for member in unevaluated {
        let len = member.regions.unevaluated_len();

        if len > 0 {
            walks.push(Walk::Unevaluated {
                owner: member.child,
                start: member.regions.unevaluated_start,
                len,
            });
        }
    }

    walks
}

#[cfg(test)]
mod grouping_selection_arithmetic {
    use super::*;

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
                group: group.clone(),
                regions: MemberRegions {
                    range_start: 0,
                    range_len,
                    trust: range_len,
                    unevaluated_start: range_len,
                    len,
                },
            }),
        }
    }

    /// Returns the regions of a child made by `member`.
    fn regions_mut(child: &mut Child) -> &mut MemberRegions {
        match &mut child.metadata {
            NodeMetadata::Window(window) => &mut window.regions,
            _ => unreachable!(),
        }
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

        assert_eq!(alternative.group().walks, [Walk::Aligned { len: 4 }]);
        assert_eq!(alternative.group().member_mask(), 0b011);
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
        *regions_mut(&mut a) = MemberRegions {
            range_start: 0,
            range_len: 6,
            trust: 2,
            unevaluated_start: 7,
            len: 9,
        };
        *regions_mut(&mut b) = MemberRegions {
            range_start: 1,
            range_len: 5,
            trust: 3,
            unevaluated_start: 6,
            len: 12,
        };

        let selection = select(&mut [a, b]);
        let alternative = selection.cheapest().unwrap();

        assert_eq!(
            alternative.group().walks,
            [
                Walk::Aligned { len: 2 },
                Walk::Damaged {
                    owner: 1,
                    start: 2,
                    len: 3
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
        regions_mut(&mut a).trust = 1;
        regions_mut(&mut b).trust = 1;

        let selection = select(&mut [a, b]);

        assert_eq!(
            selection.cheapest().unwrap().group().walks,
            [
                Walk::Aligned { len: 1 },
                Walk::Damaged {
                    owner: 0,
                    start: 1,
                    len: 3
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

    /// Asserts that regions out of order or outside the storage aren't trusted.
    #[test]
    fn invalid_regions() {
        let invalid = [
            // Trust past the range's length.
            (0, 2, 3, 2, 2),
            // The range overlaps the unevaluated region.
            (0, 3, 3, 2, 3),
            // The unevaluated region starts past the storage's end.
            (0, 2, 2, 5, 4),
            // The range's end overflows.
            (usize::MAX, 2, 0, usize::MAX, usize::MAX),
        ];

        for (range_start, range_len, trust, unevaluated_start, len) in invalid {
            let regions = MemberRegions {
                range_start,
                range_len,
                trust,
                unevaluated_start,
                len,
            };

            assert!(!regions.is_valid(), "{regions:?}");
        }

        let valid = MemberRegions {
            range_start: 0,
            range_len: 2,
            trust: 1,
            unevaluated_start: 3,
            len: 3,
        };
        assert!(valid.is_valid());
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
