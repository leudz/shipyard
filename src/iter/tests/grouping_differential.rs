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
//! A failure prints the seed, the step and its mutation, and the query shape.\
//! `Scenario::replay(seed, steps)` rebuilds the exact same world to reproduce it.

use crate::component::Component;
use crate::entity_id::EntityId;
use crate::iter::planner::{NodeMetadata, Route, Walk};
use crate::iter::{IntoIter, IntoShiperator, OneOfTwo, Shiper, Shiperator};
use crate::r#mut::Mut;
use crate::sparse_set::GroupRegion;
use crate::track;
use crate::views::{View, ViewMut};
use crate::world::World;
use crate::Get;
use alloc::collections::BTreeMap;
use alloc::format;
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

/// Groups the scenarios declare, as slot masks: AB, CD, ABC, AZ and BC.
///
/// Storages can only belong to a single group, overlapping declarations are skipped.
const GROUPS: [u8; 5] = [0b00011, 0b01100, 0b00111, 0b10001, 0b00110];

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
    /// Only used to avoid overlapping declarations, the model doesn't know about layouts.
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
            world: World::new(),
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

    /// Declares the group of `mask`, unless one of its storages already is in another group.
    fn group(&mut self, mask: u8) {
        let overlaps = self
            .declared
            .iter()
            .any(|&declared| declared != mask && declared & mask != 0);
        if overlaps {
            // Unions aren't supported, `World::group` would panic.
            return;
        }

        match mask {
            0b00011 => self.world.group::<(A, B)>(),
            0b01100 => self.world.group::<(C, D)>(),
            0b00111 => self.world.group::<(A, B, C)>(),
            0b10001 => self.world.group::<(A, Z)>(),
            0b00110 => self.world.group::<(B, C)>(),
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

/// Returns the entities in the group range of `K`'s storage.
fn range_entities<K: Kind>(world: &World) -> Vec<EntityId> {
    let view = world.borrow::<View<'_, K>>().unwrap();

    view.groups.ranges.first().map_or(Vec::new(), |range| {
        view.dense[range.start..range.end()].to_vec()
    })
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
    /// Steps where an alive entity reused a deleted one's index.
    reused: usize,
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
        for walk in &winner.group().walks {
            match walk {
                Walk::Aligned { .. } => self.aligned += 1,
                Walk::Damaged { .. } => self.damaged += 1,
                Walk::Unevaluated { .. } => self.unevaluated += 1,
            }
        }
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

        for step in 0..STEPS {
            let op = scenario.step();
            let context = format!("seed {seed}, step {step} ({op:?})");

            check_reads(&scenario, &mut rng, &mut stats, &context);
            if scenario.model.keys().any(|id| id.gen() > 0) {
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
            reused,
        } = stats;
        assert!(
            [
                ordinary_won,
                grouped,
                nested,
                aligned,
                damaged,
                unevaluated,
                reused
            ]
            .iter()
            .all(|&count| count > 0),
            "{stats:?}"
        );
    }
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
    for seed in SEEDS {
        for steps in WRITE_CHECKPOINTS {
            for case in &WRITE_CASES {
                for &way in WAYS {
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
    let mut world = World::new();
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
