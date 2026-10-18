//! Randomized differential checks of iteration against an independent model.
//!
//! Each seed drives a short sequence of mutations, applied both to a `World` and to a plain map
//! from full `EntityId`s to component values. After each step, every query shape is consumed in
//! several ways and compared with what the model says it should yield.\
//! The model never iterates a storage: expected items come from scanning its rows.
//!
//! Mutations go through the public API, except `Damage`. It moves a grouped component to its
//! storage's unevaluated region, which changes no component but leaves the partially trusted
//! ranges public operations can't produce yet. That way damaged walks get compared too.
//!
//! Groups overlap freely, so regroup creates unions and rotates rings. Layouts are checked on
//! their own after each step, independently of what queries yield.
//!
//! A failure prints the seed, the step and its mutation, and the query shape.\
//! `Scenario::replay(seed, steps)` rebuilds the exact same world to reproduce it.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::get::Get;
use crate::iter::planner::{NodeMetadata, Route, Walk};
use crate::iter::{IntoIter, IntoShiperator, OneOfTwo, Shiper, Shiperator};
use crate::r#mut::Mut;
use crate::sparse_set::{GroupRegion, Ring, SparseSet};
use crate::storage::StorageId;
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::ops::Range;
#[cfg(feature = "parallel")]
use rayon::iter::ParallelIterator;

#[cfg(not(miri))]
const SEEDS: Range<u64> = 0..48;
#[cfg(not(miri))]
const STEPS: usize = 64;
#[cfg(miri)]
const SEEDS: Range<u64> = 0..2;
#[cfg(miri)]
const STEPS: usize = 20;

/// Reads are compared after every `READ_EVERY` steps. Layouts are checked after every step.
///
/// Under Miri, every traversal is slow, so is every comparison.
#[cfg(not(miri))]
const READ_EVERY: usize = 1;
#[cfg(miri)]
const READ_EVERY: usize = 10;

const _: () = assert!(
    STEPS.is_multiple_of(READ_EVERY),
    "the last step's reads are compared"
);

/// Steps after which mutable consumers are checked, on worlds replayed up to that step.
const WRITE_CHECKPOINTS: [usize; 2] = [STEPS / 2, STEPS];

/// What mutable consumers add to every component they write.
const DELTA: u32 = 1_000_000;

/// Tiny deterministic generator, SplitMix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);

        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a number in `0..n`, `n` must not be 0.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A component the scenarios add, remove and compare by value.
trait Kind: Component<Tracking = track::Untracked> + Send + Sync {
    /// Position of this component in the model's rows.
    const SLOT: usize;

    fn new(value: u32) -> Self;
    fn value(&self) -> u32;
}

macro_rules! value_kinds {
    ($($name: ident = $slot: literal),+) => {$(
        #[derive(Debug, PartialEq)]
        struct $name(u32);

        impl Component for $name {
            type Tracking = track::Untracked;
        }

        impl Kind for $name {
            const SLOT: usize = $slot;

            fn new(value: u32) -> Self {
                $name(value)
            }

            fn value(&self) -> u32 {
                self.0
            }
        }
    )+};
}

value_kinds![A = 0, B = 1, C = 2, D = 3];

/// Zero-sized, the model stores 0 as its value.
#[derive(Debug, PartialEq)]
struct Z;

impl Component for Z {
    type Tracking = track::Untracked;
}

impl Kind for Z {
    const SLOT: usize = 4;

    fn new(_: u32) -> Self {
        Z
    }

    fn value(&self) -> u32 {
        0
    }
}

const SLOTS: usize = 5;

/// Runs `$body` with `$kind` naming the component at position `$slot`.
macro_rules! with_kind {
    ($slot: expr, $kind: ident => $body: expr) => {
        match $slot {
            0 => {
                type $kind = A;
                $body
            }
            1 => {
                type $kind = B;
                $body
            }
            2 => {
                type $kind = C;
                $body
            }
            3 => {
                type $kind = D;
                $body
            }
            4 => {
                type $kind = Z;
                $body
            }
            slot => unreachable!("no component at slot {slot}"),
        }
    };
}

/// Values of an entity's components, `None` when it doesn't own one.
type Row = [Option<u32>; SLOTS];

/// Groups the scenarios declare, as slot masks: AB, CD, ABC, AZ, BC, AC and ACD.
///
/// They overlap in every way: AB and CD are disjoint until BC or ACD connects them, AB and AC
/// make ABC unions, ABC contains AB, and an `(A, B, C)` query over AB and ACD gets C as an
/// extra captain in their ABCD union.
const GROUPS: [u8; 7] = [
    0b00011, 0b01100, 0b00111, 0b10001, 0b00110, 0b00101, 0b01101,
];

/// A mutation, as applied.
#[derive(Clone, Copy, Debug)]
enum Op {
    AddEntity {
        mask: u8,
    },
    AddComponent {
        id: EntityId,
        slot: usize,
    },
    Remove {
        id: EntityId,
        slot: usize,
    },
    DeleteComponent {
        id: EntityId,
        slot: usize,
    },
    DeleteEntity {
        id: EntityId,
    },
    Write {
        id: EntityId,
        slot: usize,
    },
    Clear {
        slot: usize,
    },
    /// Deletes the components whose value is a multiple of 3.
    Retain {
        slot: usize,
    },
    Group {
        mask: u8,
    },
    Regroup,
    /// Moves `id`'s grouped component at `slot` to the unevaluated region.
    Damage {
        id: EntityId,
        slot: usize,
    },
}

/// A world and the model it has to match, mutated in lockstep.
struct Scenario {
    rng: Rng,
    world: World,
    /// Rows of the alive entities.
    model: BTreeMap<EntityId, Row>,
    /// Groups declared so far, as slot masks.
    ///
    /// Picks damaged storages and checks unions, the model doesn't know about layouts.
    declared: Vec<u8>,
    /// Every added or written component gets a new value, so a component paired with
    /// another entity's can't go unnoticed.
    last_value: u32,
}

impl Scenario {
    /// Returns the scenario of `seed`, a quarter of them start without any group.
    fn new(seed: u64) -> Scenario {
        let mut scenario = Scenario {
            rng: Rng(seed),
            world: World::new_for_test(),
            model: BTreeMap::new(),
            declared: Vec::new(),
            last_value: 0,
        };

        if !seed.is_multiple_of(4) {
            let mask = GROUPS[scenario.rng.below(GROUPS.len())];
            scenario.apply(Op::Group { mask });
        }

        scenario
    }

    /// Returns the scenario of `seed` after its first `steps` steps.
    fn replay(seed: u64, steps: usize) -> Scenario {
        let mut scenario = Scenario::new(seed);
        for _ in 0..steps {
            scenario.step();
        }

        scenario
    }

    /// Picks a mutation, applies it and returns it.
    fn step(&mut self) -> Op {
        let op = self.choose().unwrap_or_else(|| Op::AddEntity {
            mask: self.rng.below(1 << SLOTS) as u8,
        });
        self.apply(op);

        op
    }

    /// Picks a mutation, `None` for an entity insertion or when the picked one isn't possible.
    fn choose(&mut self) -> Option<Op> {
        let slot = self.rng.below(SLOTS);

        let op = match self.rng.below(100) {
            0..30 => return None,
            30..45 => Op::AddComponent {
                id: self.pick_entity()?,
                slot,
            },
            45..53 => Op::Remove {
                id: self.pick_entity()?,
                slot,
            },
            53..58 => Op::DeleteComponent {
                id: self.pick_entity()?,
                slot,
            },
            58..66 => Op::DeleteEntity {
                id: self.pick_entity()?,
            },
            66..74 => {
                let id = self.pick_entity()?;
                if self.model[&id][slot].is_none() {
                    return None;
                }

                Op::Write { id, slot }
            }
            74..76 => Op::Clear { slot },
            76..78 => Op::Retain { slot },
            78..83 => Op::Group {
                mask: GROUPS[self.rng.below(GROUPS.len())],
            },
            83..92 => Op::Regroup,
            _ => self.pick_damage()?,
        };

        Some(op)
    }

    fn pick_entity(&mut self) -> Option<EntityId> {
        if self.model.is_empty() {
            return None;
        }

        let position = self.rng.below(self.model.len());

        self.model.keys().nth(position).copied()
    }

    /// Picks a component in the range of one of the declared groups.
    fn pick_damage(&mut self) -> Option<Op> {
        let declared = self.declared.iter().fold(0, |mask, group| mask | group);
        let slot = self.rng.below(SLOTS);
        if declared & (1 << slot) == 0 {
            return None;
        }

        let in_range = with_kind!(slot, K => range_entities::<K>(&self.world));
        if in_range.is_empty() {
            return None;
        }

        let id = in_range[self.rng.below(in_range.len())];

        Some(Op::Damage { id, slot })
    }

    fn new_value(&mut self) -> u32 {
        self.last_value += 1;
        self.last_value
    }

    fn apply(&mut self, op: Op) {
        match op {
            Op::AddEntity { mask } => self.add_entity(mask),
            Op::AddComponent { id, slot } => {
                let value = self.new_value();
                with_kind!(slot, K => {
                    self.world.add_component(id, K::new(value));
                    self.model.get_mut(&id).unwrap()[slot] = Some(K::new(value).value());
                });
            }
            Op::Remove { id, slot } => {
                with_kind!(slot, K => {
                    self.world.remove::<K>(id);
                });
                self.model.get_mut(&id).unwrap()[slot] = None;
            }
            Op::DeleteComponent { id, slot } => {
                with_kind!(slot, K => self.world.delete_component::<K>(id));
                self.model.get_mut(&id).unwrap()[slot] = None;
            }
            Op::DeleteEntity { id } => {
                self.world.delete_entity(id);
                self.model.remove(&id);
            }
            Op::Write { id, slot } => {
                let value = self.new_value();
                with_kind!(slot, K => {
                    let mut view = self.world.borrow::<ViewMut<'_, K>>().unwrap();
                    *(&mut view).get(id).unwrap() = K::new(value);
                    self.model.get_mut(&id).unwrap()[slot] = Some(K::new(value).value());
                });
            }
            Op::Clear { slot } => {
                with_kind!(slot, K => self.world.borrow::<ViewMut<'_, K>>().unwrap().clear());
                for row in self.model.values_mut() {
                    row[slot] = None;
                }
            }
            Op::Retain { slot } => {
                with_kind!(slot, K => self
                    .world
                    .borrow::<ViewMut<'_, K>>()
                    .unwrap()
                    .retain(|_, component| component.value() % 3 != 0));
                for row in self.model.values_mut() {
                    row[slot] = row[slot].filter(|value| value % 3 != 0);
                }
            }
            Op::Group { mask } => self.group(mask),
            Op::Regroup => self.world.regroup(),
            Op::Damage { id, slot } => {
                with_kind!(slot, K => self
                    .world
                    .borrow::<ViewMut<'_, K>>()
                    .unwrap()
                    .sparse_set
                    .private_move_to(id, GroupRegion::Unevaluated));
            }
        }
    }

    /// Adds an entity owning the components of `mask`.
    fn add_entity(&mut self, mask: u8) {
        let mut row = [None; SLOTS];

        // An AB pair also takes the bulk path, added in one go.
        let id = if mask == GROUPS[0] {
            let (a, b) = (self.new_value(), self.new_value());
            row[A::SLOT] = Some(a);
            row[B::SLOT] = Some(b);

            self.world.add_entity((A(a), B(b)))
        } else {
            let id = self.world.add_entity(());
            for slot in (0..SLOTS).filter(|slot| mask & (1 << slot) != 0) {
                let value = self.new_value();
                with_kind!(slot, K => {
                    self.world.add_component(id, K::new(value));
                    row[slot] = Some(K::new(value).value());
                });
            }

            id
        };

        self.model.insert(id, row);
    }

    /// Declares the group of `mask`.
    fn group(&mut self, mask: u8) {
        match mask {
            0b00011 => self.world.group::<(A, B)>(),
            0b01100 => self.world.group::<(C, D)>(),
            0b00111 => self.world.group::<(A, B, C)>(),
            0b10001 => self.world.group::<(A, Z)>(),
            0b00110 => self.world.group::<(B, C)>(),
            0b00101 => self.world.group::<(A, C)>(),
            0b01101 => self.world.group::<(A, C, D)>(),
            _ => unreachable!("not one of `GROUPS`"),
        };

        if !self.declared.contains(&mask) {
            self.declared.push(mask);
        }
    }

    /// Returns the model's items for a query, `item` returns `None` for rows it doesn't match.
    ///
    /// Items are sorted by id.
    fn expect<V>(&self, item: impl Fn(&Row) -> Option<V>) -> Vec<(EntityId, V)> {
        self.model
            .iter()
            .filter_map(|(&id, row)| Some((id, item(row)?)))
            .collect()
    }
}

/// Returns the entities in the group ranges of `K`'s storage.
fn range_entities<K: Kind>(world: &World) -> Vec<EntityId> {
    let view = world.borrow::<View<'_, K>>().unwrap();

    view.dense[..view.groups.grouped_end()].to_vec()
}

/// Asserts that every storage holds exactly the model's components, `delta` added to the ones
/// of `written` for the entities `matches` accepts.
///
/// It only uses lookups, nothing here iterates.
fn assert_components(
    world: &World,
    model: &BTreeMap<EntityId, Row>,
    matches: impl Fn(&Row) -> bool,
    written: u8,
    delta: u32,
    context: &str,
) {
    for slot in 0..SLOTS {
        with_kind!(slot, K => {
            let view = world.borrow::<View<'_, K>>().unwrap();
            let owners = model.values().filter(|row| row[slot].is_some()).count();
            assert_eq!(view.len(), owners, "slot {slot} length, {context}");

            for (&id, row) in model {
                let Some(value) = row[slot] else {
                    assert!(!view.contains(id), "{id:?} slot {slot}, {context}");
                    continue;
                };

                let expected = if written & (1 << slot) != 0 && matches(row) {
                    value + delta
                } else {
                    value
                };
                let actual = (&view).get(id).ok().map(|component| component.value());
                assert_eq!(actual, Some(expected), "{id:?} slot {slot}, {context}");
            }
        });
    }
}

/// Asserts that `items`, in any order, are exactly `expected`, which is sorted by id.
///
/// Duplicates are reported as such, sorting can't hide them.
fn assert_items<V: Clone + Debug + PartialEq>(
    items: &[(EntityId, V)],
    expected: &[(EntityId, V)],
    context: &str,
) {
    let mut sorted = items.to_vec();
    sorted.sort_by_key(|(id, _)| *id);

    if let Some(pair) = sorted.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        panic!("{:?} yielded twice, {context}", pair[0].0);
    }

    assert_eq!(sorted, expected, "{context}");
}

/// Grouped walks the reads saw installed, to make sure the corpus isn't only ordinary walks.
#[derive(Default, Debug)]
struct Stats {
    /// Root queries whose cheapest grouped alternative lost to their ordinary walk.
    ordinary_won: usize,
    grouped: usize,
    /// Grouped walks driven through a nested `Mixed`.
    nested: usize,
    aligned: usize,
    damaged: usize,
    unevaluated: usize,
    /// Winners walking several ranges.
    multi_range: usize,
    /// Winners walking a range whose ring is rotated in one of its captains.
    rotated: usize,
    /// Winners where a member outside the required ones captains a walk.
    extra_captain: usize,
    /// Steps where an alive entity reused a deleted one's index.
    reused: usize,
    /// Steps where a union regroup created was attached.
    unions: usize,
    /// Regroups leaving a group whose members are rotated differently.
    distinct_rotations: usize,
}

impl Stats {
    /// Records which walk `query`'s constructor reports as the winner.
    fn observe<Q: IntoShiperator>(&mut self, query: Q)
    where
        Q::Shiperator: Shiperator,
    {
        let (_, _, entities) = query.into_shiperator();
        let NodeMetadata::Mixed(selection) = &entities.metadata else {
            return;
        };

        let Some(winner) = selection.winner() else {
            if !selection.alternatives.is_empty() {
                self.ordinary_won += 1;
            }
            return;
        };

        self.grouped += 1;
        if matches!(winner.route, Route::Nested { .. }) {
            self.nested += 1;
        }
        let candidate = winner.group();
        let mut walked_ranges = Vec::new();
        for walk in &candidate.walks {
            let range = match *walk {
                Walk::Aligned { range, .. } => {
                    self.aligned += 1;
                    Some(range)
                }
                Walk::Damaged { range, .. } => {
                    self.damaged += 1;
                    Some(range)
                }
                Walk::Unevaluated { .. } => {
                    self.unevaluated += 1;
                    None
                }
            };

            if walk.captains() & !candidate.required != 0 {
                self.extra_captain += 1;
            }

            let Some(range) = range else {
                continue;
            };
            if !walked_ranges.contains(&range) {
                walked_ranges.push(range);
            }
            let is_rotated = candidate.ranges[range].members.iter().any(|member| {
                walk.captains() & (1 << member.child) != 0 && is_rotated(member.ring)
            });
            if is_rotated {
                self.rotated += 1;
            }
        }

        if walked_ranges.len() > 1 {
            self.multi_range += 1;
        }
    }
}

/// Returns `true` when `ring` holds components and its logical 0 isn't at its start.
fn is_rotated(ring: Ring) -> bool {
    ring.len > 0 && ring.wrap_offset() != 0
}

/// What the layout checks read from a storage.
struct Layout {
    storage_id: StorageId,
    dense: Vec<EntityId>,
    unevaluated_start: usize,
    /// Each range's group, ring and trust, in physical order.
    ranges: Vec<(Arc<[StorageId]>, Ring, usize)>,
}

impl Layout {
    /// Returns the ring of `group`'s range, `None` when this storage doesn't hold one.
    fn ring(&self, group: &Arc<[StorageId]>) -> Option<Ring> {
        self.ranges
            .iter()
            .find(|(range_group, _, _)| Arc::ptr_eq(range_group, group))
            .map(|&(_, ring, _)| ring)
    }

    /// Returns the entities of `group`'s range in logical order.
    fn logical(&self, group: &Arc<[StorageId]>) -> Vec<EntityId> {
        let ring = self.ring(group).unwrap();

        (0..ring.len)
            .map(|logical| self.dense[ring.physical_of(logical)])
            .collect()
    }
}

/// Returns the layout of every storage, by slot.
///
/// It also checks each storage's own layout contract in debug builds.
fn layouts(world: &World) -> Vec<Layout> {
    (0..SLOTS)
        .map(|slot| {
            with_kind!(slot, K => {
                let view = world.borrow::<View<'_, K>>().unwrap();
                #[cfg(debug_assertions)]
                view.groups.assert_invariants(view.dense.len());

                Layout {
                    storage_id: StorageId::of::<SparseSet<K>>(),
                    dense: view.dense.to_vec(),
                    unevaluated_start: view.groups.unevaluated_start,
                    ranges: view
                        .groups
                        .ranges
                        .iter()
                        .map(|range| (range.group.clone(), range.ring(), range.trust))
                        .collect(),
                }
            })
        })
        .collect()
}

/// Returns `group`'s members as a slot mask.
fn slot_mask(layouts: &[Layout], group: &[StorageId]) -> u8 {
    group.iter().fold(0, |mask, storage_id| {
        let slot = layouts
            .iter()
            .position(|layout| layout.storage_id == *storage_id)
            .expect("groups only hold the scenario's storages");

        mask | 1 << slot
    })
}

/// Returns the members of every group attached in `world`, as slot masks.
fn group_masks(world: &World) -> Vec<u8> {
    let layouts = layouts(world);

    layouts
        .iter()
        .flat_map(|layout| &layout.ranges)
        .map(|(group, _, _)| slot_mask(&layouts, group))
        .collect()
}

/// Returns `true` when `groups` join into a single set through shared members.
fn is_connected(groups: &[u8]) -> bool {
    let Some((&first, rest)) = groups.split_first() else {
        return false;
    };

    let mut reached = first;
    let mut left = rest.to_vec();
    while let Some(position) = left.iter().position(|&group| group & reached != 0) {
        reached |= left.swap_remove(position);
    }

    left.is_empty()
}

/// Asserts what group metadata guarantees after any step, independently of what queries yield.
///
/// `seen` holds every group met so far in this scenario, by identity.\
/// When the step `regrouped`, it also checks every grouped storage was fully evaluated
/// and every group's members hold the same fully trusted logical sequence.
fn check_layout(
    scenario: &Scenario,
    regrouped: bool,
    seen: &mut Vec<Arc<[StorageId]>>,
    stats: &mut Stats,
    context: &str,
) {
    let layouts = layouts(&scenario.world);

    // Every group once, by identity.
    let mut groups: Vec<Arc<[StorageId]>> = Vec::new();
    for layout in &layouts {
        for (group, _, _) in &layout.ranges {
            if !groups.iter().any(|known| Arc::ptr_eq(known, group)) {
                groups.push(group.clone());
            }
        }
    }

    for (index, group) in groups.iter().enumerate() {
        // A group is shared by all its members and no other group has the same members.
        for storage_id in group.iter() {
            let member = layouts
                .iter()
                .find(|layout| layout.storage_id == *storage_id)
                .unwrap();
            assert!(
                member.ring(group).is_some(),
                "{group:?} missing from a member, {context}"
            );
        }
        assert!(
            groups[index + 1..].iter().all(|other| **other != **group),
            "{group:?} allocated twice, {context}"
        );

        // A union is made of the declared groups it contains, connected through shared members.
        let mask = slot_mask(&layouts, group);
        if !scenario.declared.contains(&mask) {
            let contained: Vec<u8> = scenario
                .declared
                .iter()
                .copied()
                .filter(|&declared| declared & !mask == 0)
                .collect();
            let union = contained.iter().fold(0, |union, declared| union | declared);
            assert!(
                union == mask && is_connected(&contained),
                "union {mask:#07b} isn't made of declared groups, {context}"
            );
        }
    }

    // Groups are never dropped and keep their identity, even empty.
    for group in seen.iter() {
        assert!(
            groups.iter().any(|known| Arc::ptr_eq(known, group)),
            "{group:?} was dropped, {context}"
        );
    }
    for group in &groups {
        if !seen.iter().any(|known| Arc::ptr_eq(known, group)) {
            seen.push(group.clone());
        }
    }
    if groups
        .iter()
        .any(|group| !scenario.declared.contains(&slot_mask(&layouts, group)))
    {
        stats.unions += 1;
    }

    if !regrouped {
        return;
    }

    for layout in layouts.iter().filter(|layout| !layout.ranges.is_empty()) {
        assert_eq!(
            layout.unevaluated_start,
            layout.dense.len(),
            "unevaluated components left, {context}"
        );
        for (group, ring, trust) in &layout.ranges {
            assert_eq!(*trust, ring.len, "{group:?} damaged, {context}");
        }
    }

    let mut distinct_rotations = false;
    for group in &groups {
        let members: Vec<&Layout> = layouts
            .iter()
            .filter(|layout| group.contains(&layout.storage_id))
            .collect();

        let expected = members[0].logical(group);
        for member in &members[1..] {
            assert_eq!(
                member.logical(group),
                expected,
                "{group:?} members disagree, {context}"
            );
        }

        let offsets: Vec<Option<usize>> = members
            .iter()
            .map(|member| {
                let ring = member.ring(group).unwrap();
                is_rotated(ring).then(|| ring.wrap_offset())
            })
            .collect();
        if offsets.iter().any(|offset| *offset != offsets[0]) {
            distinct_rotations = true;
        }
    }
    if distinct_rotations {
        stats.distinct_rotations += 1;
    }
}

/// Asserts that no segment of `iter`'s plan is empty or walks an evaluated ungrouped region.
///
/// `storages` are the dense entities and the ungrouped region of each storage of the query.
fn assert_regions<S>(iter: &Shiper<S>, storages: &[(&[EntityId], Range<usize>)], context: &str) {
    let Some(schedule) = &iter.schedule else {
        return;
    };

    for segment in schedule.plan().segments() {
        let first = segment.cursor().as_ptr().as_ptr().cast_const();
        let (dense, ungrouped) = storages
            .iter()
            .find(|(dense, _)| dense.as_ptr_range().contains(&first))
            .expect("segments walk one of the query's storages");

        // SAFE: `first` points into `dense`, we just checked it.
        let start = usize::try_from(unsafe { first.offset_from(dense.as_ptr()) }).unwrap();
        let walked = start..start + segment.len();

        assert!(!walked.is_empty(), "empty segment, {context}");
        assert!(
            walked.end <= ungrouped.start || ungrouped.end <= walked.start,
            "segment {walked:?} walks the ungrouped region {ungrouped:?}, {context}"
        );
    }
}

/// Returns the `next` prefixes after which bulk consumption is compared, for `len` items.
///
/// Small traversals try every prefix, past their end included.
fn prefixes(rng: &mut Rng, len: usize) -> Vec<usize> {
    // Under Miri, a random prefix per traversal is all we can afford.
    if cfg!(miri) {
        return alloc::vec![rng.below(len + 2)];
    }

    if len <= 6 {
        return (0..=len + 1).collect();
    }

    alloc::vec![rng.below(len + 2), rng.below(len + 2)]
}

/// Consumes the iterators `$iter` builds in every way and compares them with `$expected`.
///
/// `$iter` is evaluated anew for each traversal. `$value` turns an item into a comparable value,
/// its parameter has to be typed.\
/// `for` gives the order: bulk consumption has to yield the same items in the same order,
/// even after a `next` prefix.
macro_rules! check {
    ($rng: expr, $context: expr, $iter: expr, $value: expr, $expected: expr $(,)?) => {{
        let rng: &mut Rng = $rng;
        let context: &str = $context;
        let value = $value;
        let expected = $expected;

        let mut items = Vec::new();
        for (id, item) in $iter.with_id() {
            items.push((id, value(item)));
        }
        assert_items(&items, &expected, context);
        let values: Vec<_> = items.iter().map(|(_, value)| value.clone()).collect();
        let ids: Vec<EntityId> = items.iter().map(|(id, _)| *id).collect();

        let (lower, upper) = $iter.size_hint();
        assert!(
            lower <= items.len() && upper.is_none_or(|upper| items.len() <= upper),
            "size hint {:?} for {} items, {context}",
            (lower, upper),
            items.len(),
        );

        let mut next = Vec::new();
        for item in $iter {
            next.push(value(item));
        }
        assert_eq!(next, values, "next, {context}");

        let fold = $iter.fold(Vec::new(), |mut acc, item| {
            acc.push(value(item));
            acc
        });
        assert_eq!(fold, values, "fold, {context}");

        let mut for_each = Vec::new();
        $iter
            .with_id()
            .for_each(|(id, item)| for_each.push((id, value(item))));
        assert_eq!(for_each, items, "WithId for_each, {context}");

        let only_ids: Vec<EntityId> = $iter.ids().collect();
        assert_eq!(only_ids, ids, "ids, {context}");

        for prefix in prefixes(rng, items.len()) {
            let mut iter = $iter.with_id();
            let mut head = Vec::new();
            for (id, item) in (&mut iter).take(prefix) {
                head.push((id, value(item)));
            }

            // Alternate between both bulk consumers.
            if prefix % 2 == 0 {
                head = iter.fold(head, |mut acc, (id, item)| {
                    acc.push((id, value(item)));
                    acc
                });
            } else {
                iter.for_each(|(id, item)| head.push((id, value(item))));
            }
            assert_eq!(head, items, "{prefix} next then bulk, {context}");
        }
    }};
}

/// Compares every read query shape with the model.
fn check_reads(scenario: &Scenario, rng: &mut Rng, stats: &mut Stats, context: &str) {
    assert_components(&scenario.world, &scenario.model, |_| false, 0, 0, context);

    check_views(scenario, rng, stats, context);
    check_mut_views(scenario, rng, context);
    check_replay(scenario, context);
    #[cfg(feature = "parallel")]
    check_parallel(scenario, context);
}

fn check_views(scenario: &Scenario, rng: &mut Rng, stats: &mut Stats, context: &str) {
    let (a, b, c, d, z) = scenario
        .world
        .borrow::<(
            View<'_, A>,
            View<'_, B>,
            View<'_, C>,
            View<'_, D>,
            View<'_, Z>,
        )>()
        .unwrap();
    let storages = [
        (&a.dense[..], a.groups.ungrouped_range()),
        (&b.dense[..], b.groups.ungrouped_range()),
        (&c.dense[..], c.groups.ungrouped_range()),
        (&d.dense[..], d.groups.ungrouped_range()),
        (&z.dense[..], z.groups.ungrouped_range()),
    ];

    stats.observe((&a, &b));
    stats.observe((&a, &b, &c));
    stats.observe((&a, &b, &c, &d));
    stats.observe(((&a, &b), &c));
    stats.observe(((&a, &b), (&c, &d)));

    assert_regions(&(&a, &b).iter(), &storages, &format!("(A, B), {context}"));
    assert_regions(
        &(&a, &b, &c).iter(),
        &storages,
        &format!("(A, B, C), {context}"),
    );
    assert_regions(
        &(&a, &b, &c, &d).iter(),
        &storages,
        &format!("(A, B, C, D), {context}"),
    );
    assert_regions(
        &((&a, &b), (&c, &d)).iter(),
        &storages,
        &format!("((A, B), (C, D)), {context}"),
    );

    check!(
        rng,
        &format!("(A, B), {context}"),
        (&a, &b).iter(),
        |(a, b): (&A, &B)| (a.0, b.0),
        scenario.expect(|row| Some((row[A::SLOT]?, row[B::SLOT]?))),
    );
    check!(
        rng,
        &format!("(B, A), {context}"),
        (&b, &a).iter(),
        |(b, a): (&B, &A)| (b.0, a.0),
        scenario.expect(|row| Some((row[B::SLOT]?, row[A::SLOT]?))),
    );
    check!(
        rng,
        &format!("(A, B, C), {context}"),
        (&a, &b, &c).iter(),
        |(a, b, c): (&A, &B, &C)| (a.0, b.0, c.0),
        scenario.expect(|row| Some((row[A::SLOT]?, row[B::SLOT]?, row[C::SLOT]?))),
    );
    check!(
        rng,
        &format!("(C, D), {context}"),
        (&c, &d).iter(),
        |(c, d): (&C, &D)| (c.0, d.0),
        scenario.expect(|row| Some((row[C::SLOT]?, row[D::SLOT]?))),
    );
    check!(
        rng,
        &format!("(B, C), {context}"),
        (&b, &c).iter(),
        |(b, c): (&B, &C)| (b.0, c.0),
        scenario.expect(|row| Some((row[B::SLOT]?, row[C::SLOT]?))),
    );
    check!(
        rng,
        &format!("(A, B, C, D), {context}"),
        (&a, &b, &c, &d).iter(),
        |(a, b, c, d): (&A, &B, &C, &D)| (a.0, b.0, c.0, d.0),
        scenario
            .expect(|row| { Some((row[A::SLOT]?, row[B::SLOT]?, row[C::SLOT]?, row[D::SLOT]?,)) }),
    );
    check!(
        rng,
        &format!("(A, Z), {context}"),
        (&a, &z).iter(),
        |(a, _): (&A, &Z)| a.0,
        scenario.expect(|row| {
            row[Z::SLOT]?;
            row[A::SLOT]
        }),
    );
    check!(
        rng,
        &format!("((A, B), C), {context}"),
        ((&a, &b), &c).iter(),
        |((a, b), c): ((&A, &B), &C)| (a.0, b.0, c.0),
        scenario.expect(|row| Some((row[A::SLOT]?, row[B::SLOT]?, row[C::SLOT]?))),
    );
    check!(
        rng,
        &format!("(C, (A, B)), {context}"),
        (&c, (&a, &b)).iter(),
        |(c, (a, b)): (&C, (&A, &B))| (a.0, b.0, c.0),
        scenario.expect(|row| Some((row[A::SLOT]?, row[B::SLOT]?, row[C::SLOT]?))),
    );
    check!(
        rng,
        &format!("((A, B), (C, D)), {context}"),
        ((&a, &b), (&c, &d)).iter(),
        |((a, b), (c, d)): ((&A, &B), (&C, &D))| (a.0, b.0, c.0, d.0),
        scenario
            .expect(|row| { Some((row[A::SLOT]?, row[B::SLOT]?, row[C::SLOT]?, row[D::SLOT]?,)) }),
    );
    check!(
        rng,
        &format!("(A, B, !C), {context}"),
        (&a, &b, !&c).iter(),
        |(a, b, ()): (&A, &B, ())| (a.0, b.0),
        scenario.expect(|row| {
            if row[C::SLOT].is_some() {
                return None;
            }

            Some((row[A::SLOT]?, row[B::SLOT]?))
        }),
    );
    // `Or` prefers its first branch, `Err` stands for the second one.
    check!(
        rng,
        &format!("(A, B | D), {context}"),
        (&a, &b | &d).iter(),
        |(a, item): (&A, OneOfTwo<&B, &D>)| match item {
            OneOfTwo::One(b) => (a.0, Ok(b.0)),
            OneOfTwo::Two(d) => (a.0, Err(d.0)),
        },
        scenario.expect(|row| {
            let item = match (row[B::SLOT], row[D::SLOT]) {
                (Some(b), _) => Ok(b),
                (None, Some(d)) => Err(d),
                (None, None) => return None,
            };

            Some((row[A::SLOT]?, item))
        }),
    );
}

/// Same as `check_views` with mutable outputs, only read.
fn check_mut_views(scenario: &Scenario, rng: &mut Rng, context: &str) {
    let (mut a, mut b, mut c) = scenario
        .world
        .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>, ViewMut<'_, C>)>()
        .unwrap();

    check!(
        rng,
        &format!("(&mut A, &mut B), {context}"),
        (&mut a, &mut b).iter(),
        |(a, b): (&mut A, &mut B)| (a.0, b.0),
        scenario.expect(|row| Some((row[A::SLOT]?, row[B::SLOT]?))),
    );
    check!(
        rng,
        &format!("((&mut A, &B), &mut C), {context}"),
        ((&mut a, &b), &mut c).iter(),
        |((a, b), c): ((&mut A, &B), &mut C)| (a.0, b.0, c.0),
        scenario.expect(|row| Some((row[A::SLOT]?, row[B::SLOT]?, row[C::SLOT]?))),
    );
}

/// Asserts that iterators built from the same `World::iter` replay the same items.
fn check_replay(scenario: &Scenario, context: &str) {
    let expected = scenario.expect(|row| Some((row[A::SLOT]?, row[B::SLOT]?, row[C::SLOT]?)));
    let mut iter = scenario.world.iter::<(&A, &B, &C)>();

    for pass in 0..2 {
        let items: Vec<_> = iter
            .iter()
            .with_id()
            .map(|(id, (a, b, c))| (id, (a.0, b.0, c.0)))
            .collect();

        assert_items(&items, &expected, &format!("replay {pass}, {context}"));
    }

    let mut iter = scenario.world.iter::<((&A, &B), &C)>();
    let items = iter
        .iter()
        .with_id()
        .fold(Vec::new(), |mut acc, (id, ((a, b), c))| {
            acc.push((id, (a.0, b.0, c.0)));
            acc
        });
    assert_items(&items, &expected, &format!("nested replay, {context}"));
}

#[cfg(feature = "parallel")]
fn check_parallel(scenario: &Scenario, context: &str) {
    let (a, b, c) = scenario
        .world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();

    let expected = scenario.expect(|row| Some((row[A::SLOT]?, row[B::SLOT]?)));
    let items: Vec<_> = (&a, &b)
        .par_iter()
        .with_id()
        .map(|(id, (a, b))| (id, (a.0, b.0)))
        .collect();
    assert_items(&items, &expected, &format!("parallel (A, B), {context}"));
    assert_eq!(
        (&a, &b).par_iter().count(),
        expected.len(),
        "parallel count, {context}"
    );

    let expected = scenario.expect(|row| Some((row[A::SLOT]?, row[B::SLOT]?, row[C::SLOT]?)));
    let items: Vec<_> = ((&a, &b), &c)
        .par_iter()
        .with_id()
        .map(|(id, ((a, b), c))| (id, (a.0, b.0, c.0)))
        .collect();
    assert_items(
        &items,
        &expected,
        &format!("parallel ((A, B), C), {context}"),
    );
}

/// Asserts that every read way agrees with the model after every step of every seed,
/// and that the corpus installs every kind of grouped walk.
#[test]
fn reads() {
    let mut stats = Stats::default();

    for seed in SEEDS {
        let mut scenario = Scenario::new(seed);
        // Prefix sampling has its own generator, replays only depend on the steps.
        let mut rng = Rng(!seed);

        let mut seen = Vec::new();

        for step in 0..STEPS {
            let attached = group_masks(&scenario.world);
            let op = scenario.step();
            let context = format!("seed {seed}, step {step} ({op:?})");

            // Declaring a group regroups, unless the same group is already attached:
            // declared before or created by regroup as a union.
            let regrouped = match op {
                Op::Regroup => true,
                Op::Group { mask } => !attached.contains(&mask),
                _ => false,
            };
            check_layout(&scenario, regrouped, &mut seen, &mut stats, &context);
            if (step + 1).is_multiple_of(READ_EVERY) {
                check_reads(&scenario, &mut rng, &mut stats, &context);
            }
            if scenario.model.keys().any(|id| id.generation() > 0) {
                stats.reused += 1;
            }
        }
    }

    // Under Miri the corpus is too small to promise every kind of walk.
    if cfg!(miri) {
        assert!(stats.grouped > 0, "{stats:?}");
    } else {
        let Stats {
            ordinary_won,
            grouped,
            nested,
            aligned,
            damaged,
            unevaluated,
            multi_range,
            rotated,
            extra_captain,
            reused,
            unions,
            distinct_rotations,
        } = stats;
        assert!(
            [
                ordinary_won,
                grouped,
                nested,
                aligned,
                damaged,
                unevaluated,
                multi_range,
                rotated,
                extra_captain,
                reused,
                unions,
                distinct_rotations,
            ]
            .iter()
            .all(|&count| count > 0),
            "{stats:?}"
        );
    }
}

/// Adds an entity owning the components of `mask` to `scenario` and returns it.
///
/// Only for scenarios that never deleted an entity, the new one has the largest id.
fn add(scenario: &mut Scenario, mask: u8) -> EntityId {
    scenario.apply(Op::AddEntity { mask });

    *scenario.model.keys().next_back().unwrap()
}

/// Returns the members of every group attached in `world` once, as sorted slot masks.
fn definitions(world: &World) -> Vec<u8> {
    let mut masks = group_masks(world);
    masks.sort_unstable();
    masks.dedup();

    masks
}

/// Asserts that promoting grouped entities into unions keeps every read in agreement with
/// the model, with valid layouts after each step:
/// - an AB entity gaining C and D goes straight to ABCD, no ABC or BCD union is made on the way
/// - AB entities gaining C then make ABC, ABCD keeps its identity
/// - ABC stays attached after one of its entities loses C
/// - regroup leaves members of a group rotated differently
#[test]
fn promotions() {
    const AB: u8 = 0b00011;
    const BC: u8 = 0b00110;
    const CD: u8 = 0b01100;

    // Seed 0 starts without any group.
    let mut scenario = Scenario::new(0);
    let mut rng = Rng(0);
    let mut stats = Stats::default();
    let mut seen = Vec::new();
    let mut run = |scenario: &mut Scenario, op: Op, regrouped: bool| {
        scenario.apply(op);
        let context = format!("promotions, {op:?}");

        check_layout(scenario, regrouped, &mut seen, &mut stats, &context);
        // Under Miri, reads are only compared once, after the last step.
        if !cfg!(miri) {
            check_reads(scenario, &mut rng, &mut stats, &context);
        }
    };

    let ab: Vec<EntityId> = (0..6).map(|_| add(&mut scenario, AB)).collect();
    for _ in 0..4 {
        add(&mut scenario, BC);
        add(&mut scenario, CD);
    }
    for mask in [AB, BC, CD] {
        run(&mut scenario, Op::Group { mask }, true);
    }

    let add_component = |id, slot| Op::AddComponent { id, slot };
    run(&mut scenario, add_component(ab[0], C::SLOT), false);
    run(&mut scenario, add_component(ab[0], D::SLOT), false);
    run(&mut scenario, Op::Regroup, true);
    assert_eq!(definitions(&scenario.world), [AB, BC, CD, AB | BC | CD]);

    for &id in &ab[1..4] {
        run(&mut scenario, add_component(id, C::SLOT), false);
    }
    run(&mut scenario, Op::Regroup, true);
    assert_eq!(
        definitions(&scenario.world),
        [AB, BC, AB | BC, CD, AB | BC | CD]
    );

    run(
        &mut scenario,
        Op::Remove {
            id: ab[1],
            slot: C::SLOT,
        },
        false,
    );
    run(&mut scenario, Op::Regroup, true);
    assert_eq!(
        definitions(&scenario.world),
        [AB, BC, AB | BC, CD, AB | BC | CD]
    );

    if cfg!(miri) {
        check_reads(&scenario, &mut rng, &mut stats, "promotions, last step");
    }
    assert!(stats.distinct_rotations > 0, "{stats:?}");
}

/// How a mutable consumer is driven.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Way {
    Next,
    Fold,
    ForEach,
    WithId,
    /// `next` for this many items, then `fold`.
    Prefix(usize),
    /// Collects every output before writing through any of them.
    Held,
    /// Two iterators from the same `World::iter`, each writes once.
    Replay,
    #[cfg(feature = "parallel")]
    Parallel,
}

const WAYS: &[Way] = &[
    Way::Next,
    Way::Fold,
    Way::ForEach,
    Way::WithId,
    Way::Prefix(1),
    Way::Prefix(5),
    Way::Held,
    Way::Replay,
    #[cfg(feature = "parallel")]
    Way::Parallel,
];

/// Drives `$query.iter()` in `$way`, calling `$write` on each item.
macro_rules! consume {
    ($way: expr, $query: expr, $write: expr) => {{
        let write = $write;

        match $way {
            Way::Next => {
                for item in $query.iter() {
                    write(item);
                }
            }
            Way::Fold => $query.iter().fold((), |(), item| write(item)),
            Way::ForEach => $query.iter().for_each(write),
            Way::WithId => $query.iter().with_id().for_each(|(_, item)| write(item)),
            Way::Prefix(prefix) => {
                let mut iter = $query.iter();
                for item in (&mut iter).take(prefix) {
                    write(item);
                }
                iter.fold((), |(), item| write(item));
            }
            Way::Held => {
                let held: Vec<_> = $query.iter().collect();
                for item in held {
                    write(item);
                }
            }
            #[cfg(feature = "parallel")]
            Way::Parallel => $query.par_iter().for_each(write),
            Way::Replay => unreachable!("replays go through `World::iter`"),
        }
    }};
}

/// A mutable query writing `DELTA` into some of its components.
struct WriteCase {
    shape: &'static str,
    /// The components an entity needs to match.
    required: u8,
    /// The components the query writes.
    written: u8,
    write: fn(&World, Way),
}

const WRITE_CASES: [WriteCase; 4] = [
    WriteCase {
        shape: "(&mut A, &mut B)",
        required: 0b00011,
        written: 0b00011,
        write: write_ab,
    },
    WriteCase {
        shape: "(&mut A, &B, &mut C)",
        required: 0b00111,
        written: 0b00101,
        write: write_abc,
    },
    WriteCase {
        shape: "((&mut A, &mut B), &mut C)",
        required: 0b00111,
        written: 0b00111,
        write: write_nested,
    },
    WriteCase {
        shape: "(&mut A, &mut Z)",
        required: 0b10001,
        written: 0b00001,
        write: write_az,
    },
];

fn write_ab(world: &World, way: Way) {
    let write = |(a, b): (&mut A, &mut B)| {
        a.0 += DELTA;
        b.0 += DELTA;
    };

    if way == Way::Replay {
        let mut iter = world.iter::<(&mut A, &mut B)>();
        iter.iter().for_each(write);
        for item in &mut iter {
            write(item);
        }
        return;
    }

    let (mut a, mut b) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, B>)>().unwrap();
    consume!(way, (&mut a, &mut b), write);
}

fn write_abc(world: &World, way: Way) {
    let write = |(a, _, c): (&mut A, &B, &mut C)| {
        a.0 += DELTA;
        c.0 += DELTA;
    };

    if way == Way::Replay {
        let mut iter = world.iter::<(&mut A, &B, &mut C)>();
        iter.iter().for_each(write);
        iter.iter().fold((), |(), item| write(item));
        return;
    }

    let (mut a, b, mut c) = world
        .borrow::<(ViewMut<'_, A>, View<'_, B>, ViewMut<'_, C>)>()
        .unwrap();
    consume!(way, (&mut a, &b, &mut c), write);
}

fn write_nested(world: &World, way: Way) {
    let write = |((a, b), c): ((&mut A, &mut B), &mut C)| {
        a.0 += DELTA;
        b.0 += DELTA;
        c.0 += DELTA;
    };

    if way == Way::Replay {
        let mut iter = world.iter::<((&mut A, &mut B), &mut C)>();
        iter.iter().for_each(write);
        iter.iter().with_id().for_each(|(_, item)| write(item));
        return;
    }

    let (mut a, mut b, mut c) = world
        .borrow::<(ViewMut<'_, A>, ViewMut<'_, B>, ViewMut<'_, C>)>()
        .unwrap();
    consume!(way, ((&mut a, &mut b), &mut c), write);
}

fn write_az(world: &World, way: Way) {
    let write = |(a, _): (&mut A, &mut Z)| a.0 += DELTA;

    if way == Way::Replay {
        let mut iter = world.iter::<(&mut A, &mut Z)>();
        iter.iter().for_each(write);
        iter.iter().for_each(write);
        return;
    }

    let (mut a, mut z) = world.borrow::<(ViewMut<'_, A>, ViewMut<'_, Z>)>().unwrap();
    consume!(way, (&mut a, &mut z), write);
}

/// Asserts that every mutable consumer writes exactly the components of the model's matches,
/// each once per pass.
///
/// Each consumer runs on its own world, replayed from the seed, so the order of the checks
/// doesn't change what they expect.
#[test]
fn writes() {
    let mut combination = 0;

    for seed in SEEDS {
        for steps in WRITE_CHECKPOINTS {
            for case in &WRITE_CASES {
                // Under Miri, each combination is driven in a single way, the ways take turns.
                // There are more combinations than ways, every way still runs.
                let ways = if cfg!(miri) {
                    &WAYS[combination % WAYS.len()..][..1]
                } else {
                    WAYS
                };
                combination += 1;

                for &way in ways {
                    let scenario = Scenario::replay(seed, steps);
                    let context = format!("seed {seed}, {steps} steps, {} {way:?}", case.shape);

                    (case.write)(&scenario.world, way);

                    let passes = if way == Way::Replay { 2 } else { 1 };
                    let matches = |row: &Row| {
                        (0..SLOTS)
                            .all(|slot| case.required & (1 << slot) == 0 || row[slot].is_some())
                    };
                    assert_components(
                        &scenario.world,
                        &scenario.model,
                        matches,
                        case.written,
                        DELTA * passes,
                        &context,
                    );
                }
            }
        }
    }
}

#[derive(Debug, PartialEq)]
struct Tracked(u32);

impl Component for Tracked {
    type Tracking = track::Modification;
}

/// 12 `(A, Tracked)` pairs are grouped, then `e10`'s A is moved to the unevaluated region and
/// `e12` is added. The grouped walks, aligned, damaged then unevaluated, beat the ordinary one.
///
/// No component is flagged as modified.
fn tracked_world() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let mut e: Vec<EntityId> = (0..12)
        .map(|i| world.add_entity((A(i), Tracked(i))))
        .collect();
    world.group::<(A, Tracked)>();
    world
        .borrow::<ViewMut<'_, A>>()
        .unwrap()
        .sparse_set
        .private_move_to(e[10], GroupRegion::Unevaluated);
    e.push(world.add_entity((A(12), Tracked(12))));
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_modified();

    (world, e)
}

/// Returns the kinds of the walks `query`'s winning grouped alternative drives with.
fn winner_walks<Q: IntoShiperator>(query: Q) -> Vec<&'static str>
where
    Q::Shiperator: Shiperator,
{
    let (_, _, entities) = query.into_shiperator();
    let NodeMetadata::Mixed(selection) = &entities.metadata else {
        return Vec::new();
    };

    selection.winner().map_or(Vec::new(), |winner| {
        winner
            .group()
            .walks
            .iter()
            .map(|walk| match walk {
                Walk::Aligned { .. } => "aligned",
                Walk::Damaged { .. } => "damaged",
                Walk::Unevaluated { .. } => "unevaluated",
            })
            .collect()
    })
}

/// Asserts that reading tracked mutable outputs over damaged grouped walks, in every way and
/// after every `next` prefix, flags nothing as modified.
#[test]
fn tracked_reads() {
    let (world, e) = tracked_world();
    let (a, mut tracked) = world
        .borrow::<(View<'_, A>, ViewMut<'_, Tracked>)>()
        .unwrap();
    assert_eq!(
        winner_walks((&a, &tracked)),
        ["aligned", "damaged", "unevaluated", "unevaluated"]
    );

    let expected: Vec<_> = (0..13).map(|i| (e[i], (i as u32, i as u32))).collect();
    let mut rng = Rng(0);
    check!(
        &mut rng,
        "tracked reads",
        (&a, &mut tracked).iter(),
        |(a, tracked): (&A, Mut<'_, Tracked>)| (a.0, tracked.0),
        expected,
    );

    assert!(e.iter().all(|&id| !tracked.is_modified(id)));
}

/// Asserts that writing through tracked outputs after a `next` prefix, over damaged grouped
/// walks, flags exactly the written components, the unwritten ones staying unflagged.
#[test]
fn tracked_writes() {
    let (world, e) = tracked_world();
    let (a, mut tracked) = world
        .borrow::<(View<'_, A>, ViewMut<'_, Tracked>)>()
        .unwrap();

    let mut iter = (&a, &mut tracked).iter();
    for (a, mut tracked) in (&mut iter).take(3) {
        if a.0 % 2 == 1 {
            tracked.0 += 100;
        }
    }
    iter.fold((), |(), (a, mut tracked)| {
        if a.0 % 2 == 1 {
            tracked.0 += 100;
        }
    });

    for (i, &id) in (0..).zip(&e) {
        let written = i % 2 == 1;
        assert_eq!(tracked.is_modified(id), written, "e{i}");
        assert_eq!(
            (&tracked).get(id).unwrap().0,
            if written { 100 + i } else { i },
            "e{i}"
        );
    }
}

/// AB and `(A, Tracked)` groups, after regroup promoted two `(A, Tracked)` entities that gained
/// a B into their union:
/// - `e0` to `e7` own `A(i)` and `Tracked(i)`, `e0` and `e1` also own a B
/// - `e8` to `e11` own `A(i)`, `B(i)` and `Tracked(i)`
///
/// No component is flagged as modified.
fn tracked_overlap_world() -> (World, Vec<EntityId>) {
    let mut world = World::new_for_test();
    let mut e: Vec<EntityId> = (0..8)
        .map(|i| world.add_entity((A(i), Tracked(i))))
        .collect();
    e.extend((8..12).map(|i| world.add_entity((A(i), B(i), Tracked(i)))));
    world.group::<(A, B)>().group::<(A, Tracked)>();

    world.add_component(e[0], B(100));
    world.add_component(e[1], B(101));
    world.regroup();
    world
        .borrow::<ViewMut<'_, Tracked>>()
        .unwrap()
        .clear_all_modified();

    (world, e)
}

/// Asserts that reading tracked mutable outputs over the `(A, Tracked)` range and the union,
/// in every way and after every `next` prefix, flags nothing as modified.
#[test]
fn tracked_overlap_reads() {
    let (world, e) = tracked_overlap_world();
    let (a, mut tracked) = world
        .borrow::<(View<'_, A>, ViewMut<'_, Tracked>)>()
        .unwrap();
    assert_eq!(winner_walks((&a, &tracked)), ["aligned", "aligned"]);

    let expected: Vec<_> = (0..12).map(|i| (e[i], (i as u32, i as u32))).collect();
    let mut rng = Rng(0);
    check!(
        &mut rng,
        "tracked overlap reads",
        (&a, &mut tracked).iter(),
        |(a, tracked): (&A, Mut<'_, Tracked>)| (a.0, tracked.0),
        expected,
    );

    assert!(e.iter().all(|&id| !tracked.is_modified(id)));
}

/// Asserts that writing through tracked outputs over the `(A, Tracked)` range and the union,
/// after a `next` prefix, flags exactly the written components.
#[test]
fn tracked_overlap_writes() {
    let (world, e) = tracked_overlap_world();
    let (a, mut tracked) = world
        .borrow::<(View<'_, A>, ViewMut<'_, Tracked>)>()
        .unwrap();

    let mut iter = (&a, &mut tracked).iter();
    for (a, mut tracked) in (&mut iter).take(5) {
        if a.0 % 2 == 1 {
            tracked.0 += 100;
        }
    }
    iter.fold((), |(), (a, mut tracked)| {
        if a.0 % 2 == 1 {
            tracked.0 += 100;
        }
    });

    for (i, &id) in (0..).zip(&e) {
        let written = i % 2 == 1;
        assert_eq!(tracked.is_modified(id), written, "e{i}");
        assert_eq!(
            (&tracked).get(id).unwrap().0,
            if written { 100 + i } else { i },
            "e{i}"
        );
    }
}

/// Asserts that a thread-local A in overlapping groups with B and C is grouped on its own:
/// after promotions into their union, pair and triple queries over it walk the groups and
/// yield exactly the entities owning every component, while the `Send + Sync` A stays apart.
#[cfg(feature = "thread_local")]
#[test]
fn thread_local_overlap() {
    use crate::AddComponent;
    use crate::borrow::NonSend;

    let mut world = World::new_for_test();
    let e: Vec<EntityId> = (0..12).map(|_| world.add_entity(())).collect();
    {
        let (mut local_a, mut b, mut c) = world
            .borrow::<(NonSend<ViewMut<'_, A>>, ViewMut<'_, B>, ViewMut<'_, C>)>()
            .unwrap();
        for (i, &id) in (0..).zip(&e) {
            local_a.add_component_unchecked(id, A(i));
            if i % 2 == 0 {
                b.add_component_unchecked(id, B(i));
            }
            if i % 3 == 0 {
                c.add_component_unchecked(id, C(i));
            }
        }
    }
    world.add_entity((A(100), B(100), C(100)));
    world.group::<(NonSend<A>, B)>().group::<(NonSend<A>, C)>();
    world.add_component(e[2], C(2));
    world.add_component(e[3], B(3));
    world.regroup();

    let (local_a, b, c) = world
        .borrow::<(NonSend<View<'_, A>>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    let owning = |has: &dyn Fn(EntityId) -> bool| -> Vec<EntityId> {
        e.iter().copied().filter(|&id| has(id)).collect()
    };

    let iter = (&*local_a, &b).iter();
    assert!(iter.schedule.is_some());
    let mut ids: Vec<EntityId> = iter
        .with_id()
        .map(|(id, (a_item, b_item))| {
            assert_eq!(
                (a_item.0, b_item.0),
                ((&*local_a).get(id).unwrap().0, (&b).get(id).unwrap().0)
            );
            id
        })
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, owning(&|id| b.contains(id)));

    let iter = (&*local_a, &b, &c).iter();
    assert!(iter.schedule.is_some());
    let mut ids: Vec<EntityId> = iter.ids().collect();
    ids.sort_unstable();
    assert_eq!(ids, owning(&|id| b.contains(id) && c.contains(id)));
}
