//! Builds every storage's final layout by moving entity ids only.
//!
//! Components and timestamps don't move and sparse arrays stay frozen until [`finish`] repairs
//! each storage. With the counts of the [`Plan`], each storage goes through four stages:
//!
//! 1. Gathering: every entity leaving a kept prefix is swapped to the logical end of the prefix,
//!    the same way in every member of the group, and every ungrouped component joining a group
//!    is swapped to the start of the ungrouped region. What remains of each prefix is its
//!    settled part, it keeps its logical order.
//! 2. Fitting: each range gets its final position and length. Its rotation keeps as much of its
//!    settled part where it is as it can, the rest of the settled part moves to its final slots.
//! 3. Partition: every other entity goes to a free slot of its final region.
//!    Entities already in their final region don't move.
//! 4. Copy: the members of each group copy the order of one of them, its reference.
//!
//! Gathering only reaches what it moves through the unevaluated regions, so trusted prefixes and
//! ungrouped regions are never scanned. Every scratch buffer scales with storages and ranges.

use super::analysis::{Discovery, Membership, Memberships, Plan};
use super::{original_row, storage_mut, storage_ptr};
use crate::ShipHashMap;
use crate::entity_id::EntityId;
use crate::sparse_set::Ring;
use crate::storage::{SBox, Storage, StorageId};
use alloc::vec::Vec;
use core::ops::Range;

/// What regroup leaves in a grouped storage.
pub(super) struct Layout {
    pub(super) storage_id: StorageId,
    /// The final position and rotation of each range, in range order.
    pub(super) rings: Vec<Ring>,
    /// The smallest range of rows containing every row whose id changed, `None` when none did.
    pub(super) envelope: Option<Range<usize>>,
}

/// Reorders the ids of every grouped storage into the layout `plan` decided,
/// and returns what [`finish`] needs to complete it.
///
/// ### Safety
///
/// - `plan` has to come from `analyze`, and the storages can't have changed since.
/// - Every grouped storage's ids are reordered without their components and timestamps.
///   Before a storage is used in any other way than reading its ids, sparse array and groups,
///   it has to be completed with [`finish`].
pub(super) unsafe fn build(
    storages: &mut ShipHashMap<StorageId, SBox>,
    storage_ids: &[StorageId],
    plan: &Plan,
) -> Vec<Layout> {
    // SAFE: our caller completes every storage with `finish` before using it otherwise,
    // which repairs every row we reorder.
    let mut builder = unsafe { Builder::open(storages, storage_ids, plan) };

    builder.gather();

    let placements: Vec<Vec<Placement>> = (0..builder.dense.len())
        .map(|index| {
            let placements = builder.placements(index);
            builder.move_settled(index, &placements);
            builder.partition(index, &placements);

            placements
        })
        .collect();

    builder.copy_references(&placements);

    builder.layouts(placements)
}

/// Moves every storage's components and timestamps to follow their ids,
/// then makes each storage's ranges the ones of its layout, fully trusted.
///
/// ### Safety
///
/// `layouts` have to come from [`build`], and the storages can't have changed since.
pub(super) unsafe fn finish(storages: &mut ShipHashMap<StorageId, SBox>, layouts: &[Layout]) {
    for layout in layouts {
        let storage = storage_mut(storages, layout.storage_id);

        if let Some(envelope) = layout.envelope.clone() {
            // SAFE: `build` only permuted the storage's ids, and its envelope holds every row
            // whose id changed, so `envelope` holds the same ids it did. Nothing touched the
            // sparse array since, each of these ids still points at its own row, inside `envelope`.
            unsafe { storage.repair_rows(envelope) };
        }

        let len = storage.dense().expect("group members are SparseSet").len();
        let groups = storage.groups_mut().expect("group members are SparseSet");
        for (range, ring) in groups.ranges.iter_mut().zip(&layout.rings) {
            range.start = ring.start;
            range.len = ring.len;
            range.shift = ring.shift;
            range.trust = ring.len;
        }
        groups.unevaluated_start = len;
    }
}

/// Where a range goes.
struct Placement {
    /// The range as regroup found it.
    old: Ring,
    /// The range as regroup leaves it.
    new: Ring,
    /// Length of the settled part, logical `0..settled` keeps its order.
    settled: usize,
    /// Position of the group's plan.
    group_index: usize,
}

impl Placement {
    /// Returns `true` when the settled entity at `logical` has to change slot.
    fn moves(&self, logical: usize) -> bool {
        logical < self.settled && self.old.physical_of(logical) != self.new.physical_of(logical)
    }

    /// Returns the logical positions of the settled entities that can have to change slot,
    /// as up to four ranges. Only the ones [`moves`](Self::moves) accepts do.
    ///
    /// The settled part is at most two physical pieces, from the old wrap point to the old end,
    /// then from the old start. The new rotation keeps a piece in place over the slots it shares
    /// with the new range, or none of it, so we skip that shared part without looking at it.
    fn movable(&self) -> [Range<usize>; 4] {
        let mut movable: [Range<usize>; 4] = Default::default();
        if self.settled == 0 {
            return movable;
        }

        let first_len = self.settled.min(self.old.len - self.old.wrap_offset());
        let pieces = [
            (0..first_len, self.old.physical_of(0)),
            (first_len..self.settled, self.old.start),
        ];

        for (position, (logical, start)) in pieces.into_iter().enumerate() {
            // The piece's slots run from `start`, these are the positions whose slot is in the new range.
            let first_shared = logical.start + self.new.start.saturating_sub(start);
            let past_shared = logical.start + self.new.end().saturating_sub(start);
            let shared = first_shared.min(logical.end)..past_shared.min(logical.end);

            // Every position of a piece moves by the same offset, so one of them tells for all.
            if !shared.is_empty() && !self.moves(shared.start) {
                movable[2 * position] = logical.start..shared.start;
                movable[2 * position + 1] = shared.end..logical.end;
            } else {
                movable[2 * position] = logical;
            }
        }

        movable
    }
}

/// Returns the ring of `len` slots at `start` keeping the most of `old`'s settled part,
/// its first `settled` logical positions, where it is.
///
/// The settled part is at most two physical pieces: from `old`'s wrap point to its end,
/// then from its start. A rotation keeps a piece in place over the slots both rings share.
/// It keeps both only if the length didn't change, otherwise the other piece moves.
fn new_ring(old: Ring, settled: usize, start: usize, len: usize) -> Ring {
    let ring = |wrap_offset: usize| Ring {
        start,
        len,
        shift: isize::try_from(wrap_offset).expect("range lengths fit in an isize"),
    };
    if len == 0 || old.len == 0 {
        return ring(0);
    }

    let new_slots = start..start + len;
    let old_wrap = old.wrap_offset();
    let first_len = settled.min(old.len - old_wrap);
    let first = old.start + old_wrap..old.start + old_wrap + first_len;
    let second = old.start..old.start + (settled - first_len);

    let both = len == old.len;
    let keep_first = overlap(&first, &new_slots)
        + if both {
            overlap(&second, &new_slots)
        } else {
            0
        };
    let keep_second =
        overlap(&second, &new_slots) + if both { overlap(&first, &new_slots) } else { 0 };

    // Logical 0 has to land where the kept piece puts it, possibly outside both rings.
    let logical_zero = if keep_second > keep_first {
        to_isize(old.start) - to_isize(old.len - old_wrap)
    } else {
        to_isize(old.start + old_wrap)
    };
    let wrap_offset = (logical_zero - to_isize(start)).rem_euclid(to_isize(len));

    ring(usize::try_from(wrap_offset).expect("the remainder is positive"))
}

/// Returns the number of slots `a` and `b` share.
fn overlap(a: &Range<usize>, b: &Range<usize>) -> usize {
    a.end.min(b.end).saturating_sub(a.start.max(b.start))
}

fn to_isize(value: usize) -> isize {
    isize::try_from(value).expect("dense lengths fit in an isize")
}

/// The free slots of a final region, as up to two physical ranges, with a cursor.
struct Free {
    parts: [Range<usize>; 2],
    /// Number of slots already holding an entity of the region.
    filled: usize,
}

impl Free {
    fn len(&self) -> usize {
        self.parts[0].len() + self.parts[1].len()
    }

    /// Returns the slot at `position`, counting over both parts.
    fn slot(&self, position: usize) -> usize {
        if position < self.parts[0].len() {
            self.parts[0].start + position
        } else {
            self.parts[1].start + position - self.parts[0].len()
        }
    }
}

/// Every grouped storage opened for reordering, and what laying them out needs.
struct Builder<'a, 'p> {
    /// Sorted by storage id.
    memberships: Memberships<'a>,
    /// The ids of each storage of `memberships`, at the same position.
    dense: Vec<&'a mut [EntityId]>,
    plan: &'p Plan,
    discovery: Discovery,
    /// The first and last row whose id changed, for each storage.
    touched: Vec<Option<(usize, usize)>>,
}

impl<'a, 'p> Builder<'a, 'p> {
    /// Lends the ids of every storage of the sorted `storage_ids` that has at least one range.
    ///
    /// ### Safety
    ///
    /// The ids are lent with `Storage::reorder_ids`. They can only be permuted, and every storage
    /// has to be repaired over its reordered rows before it's used in any other way than reading
    /// its ids, sparse array and groups.
    unsafe fn open(
        storages: &'a mut ShipHashMap<StorageId, SBox>,
        storage_ids: &[StorageId],
        plan: &'p Plan,
    ) -> Builder<'a, 'p> {
        let mut memberships = Vec::new();
        let mut dense = Vec::new();

        for &storage_id in storage_ids {
            let storage = storage_ptr(storages, storage_id);
            // SAFE: `storage` was just created. We keep one exclusive reference to each storage
            // for 'a, and they're all different storages. Our borrow of `storages` keeps anyone
            // else from getting to them, and getting the pointers of the other storages
            // doesn't touch this one.
            let storage: &'a mut dyn Storage = unsafe { &mut *storage };
            if !storage.groups().is_some_and(|groups| groups.has_ranges()) {
                continue;
            }

            // SAFE: our caller upholds `reorder_ids`'s requirements, they're the same.
            let parts = unsafe { storage.reorder_ids() }.expect("group members are SparseSet");
            memberships.push(Membership {
                storage_id,
                groups: parts.groups,
                sparse: parts.sparse,
            });
            dense.push(parts.dense);
        }

        Builder {
            touched: alloc::vec![None; dense.len()],
            memberships: Memberships {
                storages: memberships,
            },
            dense,
            plan,
            discovery: Discovery::default(),
        }
    }

    /// Returns the position of the storage with id `storage_id`.
    fn index_of(&self, storage_id: StorageId) -> usize {
        self.memberships
            .storages
            .binary_search_by_key(&storage_id, |membership| membership.storage_id)
            .expect("group members are grouped storages")
    }

    /// Returns the position of the plan of the group `entity`'s component in the storage at
    /// `index` goes to, `None` when it goes to the ungrouped region.
    fn classify(&mut self, index: usize, entity: EntityId) -> Option<usize> {
        let storage_id = self.memberships.storages[index].storage_id;
        let members = self
            .discovery
            .discover(&self.memberships, storage_id, entity)?;

        Some(
            self.plan
                .find(members)
                .expect("analysis attached every group an entity needs"),
        )
    }

    /// Swaps the ids at rows `a` and `b` of the storage at `index`.
    fn swap(&mut self, index: usize, a: usize, b: usize) {
        if a == b {
            return;
        }

        self.dense[index].swap(a, b);
        self.touch(index, a);
        self.touch(index, b);
    }

    /// Records that the id at `row` of the storage at `index` changed.
    fn touch(&mut self, index: usize, row: usize) {
        let touched = &mut self.touched[index];

        *touched = Some(match *touched {
            Some((first, last)) => (first.min(row), last.max(row)),
            None => (row, row),
        });
    }

    /// Returns the range of `group_index`'s group in the storage at `index`, as regroup found it.
    fn ring(&self, index: usize, group_index: usize) -> Ring {
        self.memberships.storages[index]
            .groups
            .range(&self.plan.groups[group_index].group)
            .expect("every member holds a range for its group")
            .ring()
    }

    /// Gathers every component leaving a kept prefix or an ungrouped region.
    ///
    /// They're the ones analysis counted, reached the same way through the unevaluated regions.
    fn gather(&mut self) {
        let plan = self.plan;
        let mut gathered = alloc::vec![0; plan.groups.len()];
        let mut promoted = alloc::vec![0; self.dense.len()];

        for index in 0..self.dense.len() {
            let membership = self.memberships.storages[index];

            // Gathering never touches the unevaluated regions, we can read them as we go.
            for row in membership.groups.unevaluated_start..self.dense[index].len() {
                let entity = self.dense[index][row];
                let Some(group_index) = self.classify(index, entity) else {
                    continue;
                };
                if !plan.is_first_unevaluated(
                    &self.memberships,
                    group_index,
                    membership.storage_id,
                    entity,
                ) {
                    continue;
                }

                for &member_id in plan.groups[group_index].group.iter() {
                    if member_id == membership.storage_id {
                        continue;
                    }

                    let member_index = self.index_of(member_id);
                    let member = self.memberships.storages[member_index];
                    let row = original_row(member.sparse, entity).expect(
                        "the entity satisfies the group, it owns a component in each member",
                    );

                    if let Some(range_index) = member.groups.range_index_of(row) {
                        let range = &member.groups.ranges[range_index];
                        let source_index = plan.group_index(&range.group);
                        let logical = range.ring().logical_of(row);

                        // The entity sits in the kept prefix of every member of the source,
                        // we gather it once for all of them, from the first.
                        if logical < plan.groups[source_index].kept && member_id == range.group[0] {
                            self.gather_kept(
                                source_index,
                                logical,
                                entity,
                                &mut gathered[source_index],
                            );
                        }
                    } else if row < member.groups.unevaluated_start {
                        self.gather_promoted(
                            member_index,
                            row,
                            entity,
                            &mut promoted[member_index],
                        );
                    }
                }
            }
        }

        debug_assert!(
            plan.groups
                .iter()
                .zip(&gathered)
                .all(|(group_plan, &gathered)| gathered == group_plan.leaving)
        );
        debug_assert!(self.memberships.storages.iter().zip(&promoted).all(
            |(membership, &promoted)| { promoted == plan.storage(membership.storage_id).promoted }
        ));
    }

    /// Moves `entity`, at `logical` in the kept prefix of the group of `group_index` when regroup
    /// started, to the logical end of the prefix in every member.
    ///
    /// The `gathered` last positions of the prefix already hold leaving entities.
    fn gather_kept(
        &mut self,
        group_index: usize,
        logical: usize,
        entity: EntityId,
        gathered: &mut usize,
    ) {
        let plan = self.plan;
        let group = &plan.groups[group_index].group;
        let kept = plan.groups[group_index].kept;
        let first = self.index_of(group[0]);
        let first_ring = self.ring(first, group_index);

        // Entities only enter the gathered area from their own starting position, and nothing
        // leaves it. So a starting position inside it means the entity is there, and one outside
        // holding another id means the entity moved there.
        if logical >= kept - *gathered
            || self.dense[first][first_ring.physical_of(logical)] != entity
        {
            return;
        }

        loop {
            let edge = kept - *gathered - 1;
            *gathered += 1;

            if edge == logical {
                return;
            }

            // An entity at the edge that leaves too joins the area where it is.
            let edge_entity = self.dense[first][first_ring.physical_of(edge)];
            if self.classify(first, edge_entity) != Some(group_index) {
                continue;
            }

            // Kept prefixes are aligned, the same swap keeps every member aligned.
            for &member_id in group.iter() {
                let member = self.index_of(member_id);
                let ring = self.ring(member, group_index);

                self.swap(member, ring.physical_of(logical), ring.physical_of(edge));
            }

            return;
        }
    }

    /// Moves `entity`, at `row` in the ungrouped region of the storage at `index` when regroup
    /// started, to the start of the region.
    ///
    /// The `promoted` first rows of the region already hold components joining a group.
    fn gather_promoted(
        &mut self,
        index: usize,
        row: usize,
        entity: EntityId,
        promoted: &mut usize,
    ) {
        let start = self.memberships.storages[index].groups.grouped_end();

        // See `gather_kept`, the area works the same way.
        if row < start + *promoted || self.dense[index][row] != entity {
            return;
        }

        loop {
            let edge = start + *promoted;
            *promoted += 1;

            if edge == row {
                return;
            }

            let edge_entity = self.dense[index][edge];
            if self.classify(index, edge_entity).is_some() {
                continue;
            }

            self.swap(index, row, edge);

            return;
        }
    }

    /// Returns where each range of the storage at `index` goes.
    fn placements(&self, index: usize) -> Vec<Placement> {
        let mut start = 0;

        self.memberships.storages[index]
            .groups
            .ranges
            .iter()
            .map(|range| {
                let group_index = self.plan.group_index(&range.group);
                let group_plan = &self.plan.groups[group_index];
                let settled = group_plan.kept - group_plan.leaving;
                let len = settled + group_plan.incoming;
                let old = range.ring();
                let new = new_ring(old, settled, start, len);
                start += len;

                Placement {
                    old,
                    new,
                    settled,
                    group_index,
                }
            })
            .collect()
    }

    /// Moves every settled entity of the storage at `index` that its range's new ring doesn't
    /// keep in place to its new slot.
    ///
    /// Each one's target slot either holds an entity that isn't settled, or a settled one that
    /// moves too. These form chains, which we follow from their end, the move whose target holds
    /// no settled entity, back to their start. Each swap then puts a settled entity in its slot and
    /// leaves an entity that isn't settled where it was, for the next one up the chain to take.
    fn move_settled(&mut self, index: usize, placements: &[Placement]) {
        let mut expected = 0;
        let mut moved = 0;

        for placement in placements {
            for logical in placement.movable().into_iter().flatten() {
                if !placement.moves(logical) {
                    continue;
                }
                expected += 1;

                let target = placement.new.physical_of(logical);
                if moving_from(placements, target).is_some() {
                    continue;
                }

                let mut current = (placement, logical);
                loop {
                    let (placement, logical) = current;
                    let source = placement.old.physical_of(logical);
                    self.swap(index, source, placement.new.physical_of(logical));
                    moved += 1;

                    match moving_to(placements, source) {
                        Some(previous) => current = previous,
                        None => break,
                    }
                }
            }
        }

        // Ranges keep their order and each keeps a piece of its settled part in place,
        // so these moves never go around in a cycle.
        debug_assert_eq!(moved, expected);
    }

    /// Moves every entity that isn't settled to a free slot of its final region,
    /// in the storage at `index`.
    ///
    /// Ranges' free slots are their logical positions past the settled part. The ungrouped
    /// region's are all its slots, except the ones still holding the ungrouped components
    /// that stay. We never look at those.
    fn partition(&mut self, index: usize, placements: &[Placement]) {
        let membership = self.memberships.storages[index];
        let len = self.dense[index].len();

        let mut regions: Vec<Free> = placements
            .iter()
            .map(|placement| {
                let free = placement.new.len - placement.settled;
                if free == 0 {
                    return Free {
                        parts: [0..0, 0..0],
                        filled: 0,
                    };
                }

                let first = placement.new.physical_of(placement.settled);
                let first_len = placement.new.contiguous_len(placement.settled);

                Free {
                    parts: [
                        first..first + first_len,
                        placement.new.start..placement.new.start + (free - first_len),
                    ],
                    filled: 0,
                }
            })
            .collect();

        let ungrouped_start = placements.last().map_or(0, |placement| placement.new.end());
        let staying_start =
            membership.groups.grouped_end() + self.plan.storage(membership.storage_id).promoted;
        let staying = staying_start.clamp(ungrouped_start, len)
            ..membership
                .groups
                .unevaluated_start
                .clamp(ungrouped_start, len);
        regions.push(Free {
            parts: [ungrouped_start..staying.start, staying.end..len],
            filled: 0,
        });

        for region in 0..regions.len() {
            while regions[region].filled < regions[region].len() {
                let slot = regions[region].slot(regions[region].filled);
                let destination = self.region_of(index, self.dense[index][slot], placements);

                if destination == region {
                    regions[region].filled += 1;
                    continue;
                }

                // The destination's next slot holding an entity that belongs elsewhere.
                // Earlier regions are complete, the entity can only belong to a later one.
                let target = loop {
                    let free = &regions[destination];
                    assert!(
                        free.filled < free.len(),
                        "the plan counts every entity of a region"
                    );
                    let target = free.slot(free.filled);

                    if self.region_of(index, self.dense[index][target], placements) != destination {
                        break target;
                    }
                    regions[destination].filled += 1;
                };

                self.swap(index, slot, target);
                regions[destination].filled += 1;
            }
        }
    }

    /// Returns the position of the region `entity`'s component in the storage at `index` goes to,
    /// one past the ranges for the ungrouped region.
    fn region_of(&mut self, index: usize, entity: EntityId, placements: &[Placement]) -> usize {
        let Some(group_index) = self.classify(index, entity) else {
            return placements.len();
        };

        placements
            .iter()
            .position(|placement| placement.group_index == group_index)
            .expect("every member holds a range for its group")
    }

    /// Makes every member of each group copy the order of its reference,
    /// past the settled part they share.
    ///
    /// The reference is the member that trusted the most of its range, the first one on ties.
    /// Every member holds the same entities, so each copy only reorders them.
    fn copy_references(&mut self, placements: &[Vec<Placement>]) {
        let plan = self.plan;

        for (group_index, group_plan) in plan.groups.iter().enumerate() {
            let placement_of = |member: usize| {
                placements[member]
                    .iter()
                    .find(|placement| placement.group_index == group_index)
                    .expect("every member holds a range for its group")
            };

            let mut reference = self.index_of(group_plan.group[0]);
            let mut reference_trust = 0;
            for &member_id in group_plan.group.iter() {
                let member = self.index_of(member_id);
                let trust = self.memberships.storages[member]
                    .groups
                    .range(&group_plan.group)
                    .expect("every member holds a range for its group")
                    .trust;

                if trust > reference_trust {
                    reference = member;
                    reference_trust = trust;
                }
            }

            let source = placement_of(reference);
            for &member_id in group_plan.group.iter() {
                let member = self.index_of(member_id);
                if member == reference {
                    continue;
                }

                let target = placement_of(member);
                debug_assert_eq!(target.settled, source.settled);
                debug_assert_eq!(target.new.len, source.new.len);

                for logical in source.settled..source.new.len {
                    let entity = self.dense[reference][source.new.physical_of(logical)];
                    let row = target.new.physical_of(logical);

                    if self.dense[member][row] != entity {
                        self.dense[member][row] = entity;
                        self.touch(member, row);
                    }
                }
            }
        }
    }

    /// Returns each storage's layout.
    fn layouts(self, placements: Vec<Vec<Placement>>) -> Vec<Layout> {
        self.memberships
            .storages
            .iter()
            .zip(placements)
            .zip(self.touched)
            .map(|((membership, placements), touched)| Layout {
                storage_id: membership.storage_id,
                rings: placements
                    .into_iter()
                    .map(|placement| placement.new)
                    .collect(),
                envelope: touched.map(|(first, last)| first..last + 1),
            })
            .collect()
    }
}

/// Returns the settled entity that has to leave `slot`, as its range's placement and its logical
/// position, if any.
fn moving_from(placements: &[Placement], slot: usize) -> Option<(&Placement, usize)> {
    let placement = placements
        .iter()
        .find(|placement| placement.old.contains(slot))?;
    let logical = placement.old.logical_of(slot);

    placement.moves(logical).then_some((placement, logical))
}

/// Returns the settled entity that has to move to `slot`, as its range's placement and its logical
/// position, if any.
fn moving_to(placements: &[Placement], slot: usize) -> Option<(&Placement, usize)> {
    let placement = placements
        .iter()
        .find(|placement| placement.new.contains(slot))?;
    let logical = placement.new.logical_of(slot);

    placement.moves(logical).then_some((placement, logical))
}
