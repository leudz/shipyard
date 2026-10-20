//! Worlds built through the public API, with an independent model of what each query yields.
//!
//! Every fixture records the entities its query should yield and the values they were given,
//! straight from construction. Checks compare iteration against that model, never against
//! another iteration.

use shipyard::{Component, EntityId, Get, View, ViewMut, World, track};
use std::collections::HashMap;
use std::ops::Range;

/// Entities in the dense and aligned fixtures. Not a multiple of any vector width,
/// so vector loops also run their scalar remainder.
pub const N: u32 = 100_003;
/// Matches in the sparse and nested fixtures.
pub const SPARSE: u32 = 10_003;
/// C components in the small captain fixture.
pub const SMALL_CAPTAIN: u32 = 1_003;
/// Pairs in the construction fixtures.
pub const SMALL_QUERY: u32 = 103;
/// Size of each late entity set in the coverage fixture.
pub const LATE: u32 = 10_003;
/// In the multiple-group fixtures, entities created in AB, in ABC and in AC.
pub const MULTI_AB: u32 = 40_003;
pub const MULTI_ABC: u32 = 30_003;
pub const MULTI_AC: u32 = 24_003;
/// In the multiple-group fixtures, AB pairs gaining a C and AC pairs gaining a B after grouping.
pub const JOINS: u32 = 4_003;
/// Pairs losing their B, then new pairs, in the damage fixtures.
/// Also the components the lifecycle cases remove.
pub const CHURN: u32 = 1_003;
/// Pairs the large regroup case changes, a quarter of `N`.
pub const QUARTER: u32 = N / 4;
/// Words of [`H`]'s payload.
const HEAVY_WORDS: usize = 32;
/// Pairs in front of the tail fixture's large segment.
#[cfg(feature = "parallel")]
pub const HEAD: u32 = 3;

pub struct A(pub u32);
impl Component for A {
    type Tracking = track::Untracked;
}

pub struct B(pub u32);
impl Component for B {
    type Tracking = track::Untracked;
}

pub struct C(pub u32);
impl Component for C {
    type Tracking = track::Untracked;
}

/// A large component, 256 bytes, so moving it costs much more than moving its entity.
pub struct H(pub [u64; HEAVY_WORDS]);
impl Component for H {
    type Tracking = track::Untracked;
}

// Values are a function of a per-fixture logical index. They're nonconstant so a wrong
// pairing changes the checksums, and they use the full 32 bits so wrapping happens.

pub fn a_value(i: u32) -> u32 {
    i.wrapping_mul(0x9E37_79B9) ^ 0x0F0F_1234
}

pub fn b_value(i: u32) -> u32 {
    i.wrapping_mul(0x85EB_CA6B).wrapping_add(0x2545_F491)
}

pub fn c_value(i: u32) -> u32 {
    i.wrapping_mul(0xC2B2_AE35) ^ 0x6A09_E667
}

/// The H of an entity whose A holds `a`.
pub fn h_value(a: u32) -> [u64; HEAVY_WORDS] {
    std::array::from_fn(|word| (a as u64) << 8 | word as u64)
}

/// The value the mutable consumers write to A, from the unchanged B.
///
/// It doesn't read A, so running a mutable traversal any number of times leaves the same values.
pub fn mutated_a(b: u32) -> u32 {
    b.wrapping_mul(3) ^ 0xA5A5
}

/// Spreads `count` picks evenly over `0..total`, returning the `i`th one.
///
/// Distinct for distinct `i` as long as `count <= total`.
fn spread(i: u32, count: u32, total: u32) -> u32 {
    (i as u64 * total as u64 / count as u64) as u32
}

/// An entity owning an A, with the index its values were built from and its other components.
#[derive(Clone, Copy)]
struct Owner {
    id: EntityId,
    i: u32,
    b: bool,
    c: bool,
}

/// An entity the fixture's query should yield, with the values it was given.
#[derive(Clone, Copy)]
pub struct Match {
    pub id: EntityId,
    pub a: u32,
    pub b: u32,
    /// Only set when the query reads C.
    pub c: Option<u32>,
}

/// A world and the model of its query.
pub struct Fixture {
    pub world: World,
    /// What the query yields, in any order.
    pub matches: Vec<Match>,
    /// Entities owning an A the query never yields, with their A.
    pub unmatched_a: Vec<(EntityId, u32)>,
    /// The walk the planner should pick, as `size_hint` reports it: the candidates it visits
    /// and whether that's also exactly the number of items.
    pub candidates: usize,
    pub exact: bool,
}

impl Fixture {
    fn new(world: World, candidates: u32, exact: bool) -> Fixture {
        Fixture {
            world,
            matches: Vec::new(),
            unmatched_a: Vec::new(),
            candidates: candidates as usize,
            exact,
        }
    }

    /// Adds an entity owning A and B, which the query yields.
    fn add_pair(&mut self, i: u32) -> EntityId {
        let (a, b) = (a_value(i), b_value(i));
        let id = self.world.add_entity((A(a), B(b)));
        self.matches.push(Match { id, a, b, c: None });
        id
    }

    /// Adds an entity owning A only, which the query never yields.
    fn add_lone_a(&mut self, i: u32) {
        let a = a_value(i);
        let id = self.world.add_entity(A(a));
        self.unmatched_a.push((id, a));
    }

    /// Gives C to the entity built from index `i`, which then becomes a match of the query.
    fn add_c(&mut self, id: EntityId, i: u32) {
        let c = c_value(i);
        self.world.add_component(id, C(c));
        self.matches.push(Match {
            id,
            a: a_value(i),
            b: b_value(i),
            c: Some(c),
        });
    }

    pub fn yielded(&self) -> usize {
        self.matches.len()
    }

    /// The `size_hint` of the walk the planner should pick.
    pub fn expected_hint(&self) -> (usize, Option<usize>) {
        let lower = if self.exact { self.candidates } else { 0 };
        (lower, Some(self.candidates))
    }

    /// Sorted ids of the matches.
    pub fn ids(&self) -> Vec<EntityId> {
        let mut ids: Vec<_> = self.matches.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        ids
    }

    /// What the pair read consumers return.
    pub fn pair_sum(&self) -> u32 {
        let mut sum = 0u32;
        for m in &self.matches {
            sum = sum.wrapping_add(m.a ^ (m.b << 3));
        }
        sum
    }

    /// What the pair ID consumers return.
    pub fn pair_id_sum(&self) -> u32 {
        let mut sum = 0u32;
        for m in &self.matches {
            let id_hash = (m.id.index() as u32).wrapping_mul(0x9E37_79B9);
            sum = sum.wrapping_add(id_hash ^ m.a ^ (m.b << 3));
        }
        sum
    }

    /// What the triple and nested read consumers return.
    pub fn triple_sum(&self) -> u32 {
        let mut sum = 0u32;
        for m in &self.matches {
            let c = m.c.expect("triple fixtures give every match a C");
            sum = sum.wrapping_add(m.a ^ (m.b << 3) ^ (c << 7));
        }
        sum
    }

    /// What the triple read consumers return over the matches owning a C, the others skipped.
    pub fn union_sum(&self) -> u32 {
        let mut sum = 0u32;
        for m in &self.matches {
            if let Some(c) = m.c {
                sum = sum.wrapping_add(m.a ^ (m.b << 3) ^ (c << 7));
            }
        }
        sum
    }

    /// Records `owners` as matches when they own every component the query reads,
    /// as unmatched A otherwise.
    fn model(&mut self, owners: &[Owner], reads_c: bool) {
        for owner in owners {
            if owner.b && (owner.c || !reads_c) {
                self.matches.push(Match {
                    id: owner.id,
                    a: a_value(owner.i),
                    b: b_value(owner.i),
                    c: reads_c.then(|| c_value(owner.i)),
                });
            } else {
                self.unmatched_a.push((owner.id, a_value(owner.i)));
            }
        }
    }

    /// `count` matches spread evenly over the model, starting with the first one.
    pub fn spread_matches(&self, count: u32) -> Vec<Match> {
        let total = self.yielded() as u32;
        (0..count)
            .map(|k| self.matches[spread(k, count, total) as usize])
            .collect()
    }

    /// The last `count` matches.
    ///
    /// A freshly grouped fixture holds its matches in insertion order, so these are at the
    /// logical end of the group's range.
    pub fn tail_matches(&self, count: u32) -> Vec<Match> {
        self.matches[self.matches.len() - count as usize..].to_vec()
    }

    /// `CHURN` matches spread over the fixture, with their B, for the lifecycle cases to delete
    /// and restore.
    pub fn churned(&self) -> Vec<(EntityId, u32)> {
        self.spread_matches(CHURN)
            .iter()
            .map(|m| (m.id, m.b))
            .collect()
    }

    /// Gives back their B to pairs [`churned`](Fixture::churned) returned.
    pub fn restore_b(&mut self, pairs: &[(EntityId, u32)]) {
        for &(id, b) in pairs {
            self.world.add_component(id, B(b));
        }
    }

    /// Makes `churn`'s changes to the world and to the model.
    ///
    /// A match losing a component the query reads stops being a match. `candidates` and `exact`
    /// are left as they are, what a query walks depends on whether the world gets regrouped.
    pub fn churn(&mut self, churn: &Churn) {
        let positions: HashMap<EntityId, usize> = (0..)
            .zip(&self.matches)
            .map(|(position, m)| (m.id, position))
            .collect();
        let mut lost = vec![false; self.matches.len()];

        for &(id, change) in &churn.changes {
            let position = positions[&id];
            let m = &mut self.matches[position];

            match change {
                Change::RemoveA | Change::Delete => lost[position] = true,
                Change::RemoveB | Change::RemoveH => {
                    lost[position] = true;
                    self.unmatched_a.push((id, m.a));
                }
                Change::ReaddA(a) => m.a = a,
                Change::ReaddB(b) => m.b = b,
                Change::AddC(c) => m.c = Some(c),
                Change::RemoveC => m.c = None,
            }
        }

        let mut lost = lost.into_iter();
        self.matches.retain(|_| !lost.next().unwrap());

        let matches = &mut self.matches;
        churn.apply(&mut self.world, |id, i| {
            matches.push(Match {
                id,
                a: a_value(i),
                b: b_value(i),
                c: None,
            });
        });
    }

    /// Asserts that A and B hold the union's matches, the ones owning a C, in different
    /// physical orders.
    ///
    /// Members share the union's logical order, so their rings wrap at different points
    /// and the union's walk is cut in several segments.
    pub fn check_wraps(&self, a: &View<'_, A>, b: &View<'_, B>, c: &View<'_, C>, case: &str) {
        let union: Vec<EntityId> = self
            .matches
            .iter()
            .map(|m| m.id)
            .filter(|&id| c.contains(id))
            .collect();
        assert!(!union.is_empty(), "{case}: empty union");

        let physical_order = |index_of: &dyn Fn(EntityId) -> Option<usize>| {
            let mut ids = union.clone();
            ids.sort_by_key(|&id| index_of(id).unwrap());
            ids
        };
        let a_order = physical_order(&|id| a.index_of(id));
        let b_order = physical_order(&|id| b.index_of(id));
        assert!(a_order != b_order, "{case}: A and B wrap at the same point");
    }

    /// Asserts that every matched A holds `mutated_a(b)` and every unmatched A is unchanged.
    pub fn check_mutated(&self, a: &ViewMut<'_, A>, case: &str) {
        for m in &self.matches {
            let value = a.get(m.id).unwrap().0;
            assert_eq!(value, mutated_a(m.b), "{case}: matched A not written");
        }
        for &(id, initial) in &self.unmatched_a {
            let value = a.get(id).unwrap().0;
            assert_eq!(value, initial, "{case}: unmatched A written");
        }
    }
}

/// `N` ungrouped AB pairs. A captains, B is looked up.
///
/// Candidates: `N`. Yields: `N`.
pub fn dense() -> Fixture {
    let mut fixture = Fixture::new(World::new(), N, false);
    for i in 0..N {
        fixture.add_pair(i);
    }
    fixture
}

/// `N` A components, `SPARSE` of them spread over the storage also own a B.
/// No group. B captains, A is looked up.
///
/// Candidates: `SPARSE`. Yields: `SPARSE`.
pub fn sparse() -> Fixture {
    let mut fixture = Fixture::new(World::new(), SPARSE, false);
    let ids: Vec<_> = (0..N)
        .map(|i| fixture.world.add_entity(A(a_value(i))))
        .collect();

    let mut has_b = vec![false; N as usize];
    for k in 0..SPARSE {
        has_b[spread(k, SPARSE, N) as usize] = true;
    }

    for (i, (&id, has_b)) in (0..).zip(ids.iter().zip(has_b)) {
        if has_b {
            let b = b_value(i);
            fixture.world.add_component(id, B(b));
            fixture.matches.push(Match {
                id,
                a: a_value(i),
                b,
                c: None,
            });
        } else {
            fixture.unmatched_a.push((id, a_value(i)));
        }
    }

    fixture
}

/// Adds `N` AB pairs, then gives a C to `count` of them spread over the storages.
///
/// Only the pairs owning a C are matches.
fn pairs_with_c(fixture: &mut Fixture, count: u32) {
    let ids: Vec<_> = (0..N)
        .map(|i| fixture.world.add_entity((A(a_value(i)), B(b_value(i)))))
        .collect();

    for k in 0..count {
        let i = spread(k, count, N);
        fixture.add_c(ids[i as usize], i);
    }
}

/// `N` AB pairs, `SPARSE` of them spread over the storages also own a C. No group.
/// For `((&A, &B), &C)`: C captains, the nested AB is looked up.
///
/// Candidates: `SPARSE`. Yields: `SPARSE`.
pub fn nested() -> Fixture {
    let mut fixture = Fixture::new(World::new(), SPARSE, false);
    pairs_with_c(&mut fixture, SPARSE);
    fixture
}

/// `N` grouped AB pairs, `SMALL_CAPTAIN` of them spread over the storages also own a C.
///
/// For `(&A, &B, &C)` the ordinary walk with C captaining costs `1_003 * (2 + 2)`, the aligned
/// group walk with C looked up `N * (2 + 1)`. Ordinary wins.
///
/// Candidates: `SMALL_CAPTAIN`. Yields: `SMALL_CAPTAIN`.
pub fn small_captain() -> Fixture {
    let mut fixture = Fixture::new(World::new(), SMALL_CAPTAIN, false);
    pairs_with_c(&mut fixture, SMALL_CAPTAIN);
    fixture.world.group::<(A, B)>();
    fixture
}

/// `N` AB pairs, 3 A-only and 3 B-only entities, grouped. The AB group is a single
/// aligned segment, the lone components sit in the ungrouped regions and aren't walked.
///
/// Candidates: `N`, exact. Yields: `N`.
pub fn aligned() -> Fixture {
    aligned_pairs(N)
}

/// `count` AB pairs, 3 A-only and 3 B-only entities, grouped.
///
/// Candidates: `count`, exact. Yields: `count`.
fn aligned_pairs(count: u32) -> Fixture {
    let mut fixture = ungrouped_pairs(count);
    fixture.world.group::<(A, B)>();
    fixture
}

/// `aligned_pairs`'s entities, without any group yet.
///
/// The candidates are the grouped ones: `count`, exact. Yields: `count`.
fn ungrouped_pairs(count: u32) -> Fixture {
    let mut fixture = Fixture::new(World::new(), count, true);
    for i in 0..count {
        fixture.add_pair(i);
    }
    for i in count..count + 3 {
        fixture.add_lone_a(i);
        fixture.world.add_entity(B(b_value(i)));
    }
    fixture
}

/// `aligned`'s entities, without any group yet, for the case declaring it.
///
/// Once grouped, candidates: `N`, exact. Yields: `N`.
pub fn ungrouped() -> Fixture {
    ungrouped_pairs(N)
}

/// An aligned prefix, then late matches in A's and B's unevaluated regions.
///
/// Built like the private `coverage_world` fixture, scaled up:
///
/// 1. `N` AB pairs and `LATE` A-only entities, the "pre" set, then the AB group.
///    The pre set is evaluated as ungrouped.
/// 2. `LATE` more A-only entities, the "post" set. They land in A's unevaluated region.
/// 3. B is added to both sets, alternating between pre and post.
///    Every one of those B lands in B's unevaluated region.
/// 4. `LATE` A-only entities that never match, also in A's unevaluated region.
///
/// Both unevaluated regions hold `2 * LATE` components. The walks are:
///
/// - aligned: `N` candidates, `N` items
/// - A's unevaluated region, B looked up: `2 * LATE` candidates, the post set's `LATE` items
/// - B's unevaluated region, A looked up and checked against A's unevaluated region:
///   `2 * LATE` candidates, the pre set's `LATE` items, the post set covered
///
/// The pre set's A components are never walked as a segment of their own.
///
/// Candidates: `N + 4 * LATE` (140,015). Yields: `N + 2 * LATE` (120,009).
/// With B captaining alone the ordinary walk would cost `120_009 * 3`, the grouped walks cost
/// `N * 2 + 4 * LATE * 3`.
pub fn coverage() -> Fixture {
    let mut fixture = Fixture::new(World::new(), N + 4 * LATE, false);
    for i in 0..N {
        fixture.add_pair(i);
    }
    let pre: Vec<_> = (N..N + LATE)
        .map(|i| fixture.world.add_entity(A(a_value(i))))
        .collect();
    fixture.world.group::<(A, B)>();
    let post: Vec<_> = (N + LATE..N + 2 * LATE)
        .map(|i| fixture.world.add_entity(A(a_value(i))))
        .collect();

    for k in 0..LATE {
        for (set, start) in [(&pre, N), (&post, N + LATE)] {
            let (id, i) = (set[k as usize], start + k);
            let b = b_value(i);
            fixture.world.add_component(id, B(b));
            fixture.matches.push(Match {
                id,
                a: a_value(i),
                b,
                c: None,
            });
        }
    }

    for i in N + 2 * LATE..N + 3 * LATE {
        fixture.add_lone_a(i);
    }

    fixture
}

/// `N` AB pairs, every second one also owning a C, then enough C-only entities to make C
/// the largest storage. Grouped AB.
///
/// For `(&A, &B, &C)` the ordinary walk with A captaining costs `N * (2 + 2)`, the aligned
/// group walk with C looked up `N * (2 + 1)`. The group wins, in its partial strategy:
/// A and B drive and C rejects half the candidates.
///
/// Candidates: `N`. Yields: `(N + 1) / 2` (50,002).
pub fn partial() -> Fixture {
    let mut fixture = Fixture::new(World::new(), N, false);
    let ids: Vec<_> = (0..N)
        .map(|i| fixture.world.add_entity((A(a_value(i)), B(b_value(i)))))
        .collect();
    for i in (0..N).step_by(2) {
        fixture.add_c(ids[i as usize], i);
    }

    let lone_c = N - fixture.yielded() as u32 + 1_000;
    for i in N..N + lone_c {
        fixture.world.add_entity(C(c_value(i)));
    }
    fixture.world.group::<(A, B)>();

    fixture
}

/// `SMALL_QUERY` ungrouped AB pairs.
///
/// Candidates: `SMALL_QUERY`. Yields: `SMALL_QUERY`.
pub fn small_ordinary() -> Fixture {
    let mut fixture = Fixture::new(World::new(), SMALL_QUERY, false);
    for i in 0..SMALL_QUERY {
        fixture.add_pair(i);
    }
    fixture
}

/// `SMALL_QUERY` AB pairs, 3 A-only and 3 B-only entities, grouped.
///
/// Candidates: `SMALL_QUERY`, exact. Yields: `SMALL_QUERY`.
pub fn small_grouped() -> Fixture {
    aligned_pairs(SMALL_QUERY)
}

/// `small_grouped`'s world, for `(&A, &B, !&C)` where no entity owns a C.
///
/// The filter keeps ordinary selection even though A and B are grouped: A captains over the
/// pairs and its lone components, B is looked up.
///
/// Candidates: `SMALL_QUERY + 3`. Yields: `SMALL_QUERY`.
pub fn small_filtered() -> Fixture {
    let mut fixture = aligned_pairs(SMALL_QUERY);
    fixture.candidates += 3;
    fixture.exact = false;
    fixture
}

/// `SMALL_QUERY` AB pairs, every second one also owning a C, grouped as `(A, B, C)`.
///
/// For `(&A, &B)` the ABC group doesn't hold every match. The query reads the group metadata
/// and walks ordinarily: A captains, B is looked up.
///
/// Candidates: `SMALL_QUERY`. Yields: `SMALL_QUERY`.
pub fn small_projection() -> Fixture {
    let mut fixture = Fixture::new(World::new(), SMALL_QUERY, false);
    for i in 0..SMALL_QUERY {
        let id = fixture.add_pair(i);
        if i % 2 == 0 {
            fixture.world.add_component(id, C(c_value(i)));
        }
    }
    fixture.world.group::<(A, B, C)>();
    fixture
}

/// Whether a fixture declares its groups, or builds the same entities without any as a control.
#[derive(Clone, Copy, PartialEq)]
pub enum Layout {
    Grouped,
    Ordinary,
}

/// AB and AC groups, overlapping on A, and the ABC union they generate.
///
/// 1. `MULTI_AB` AB pairs, `MULTI_ABC` ABC triples, `MULTI_AC` AC pairs and 3 B-only entities.
/// 2. `(A, B)` then `(A, C)` are grouped, the ABC triples land in the union.
/// 3. `JOINS` AB pairs gain a C and `JOINS` AC pairs a B, spread over their sets.
/// 4. A regroup promotes them into the union.
///
/// Values are built from each entity's index in the returned owners.
/// An ordinary layout creates and mutates the same entities, without any group or regroup.
fn multiple_world(layout: Layout) -> (World, Vec<Owner>) {
    let mut world = World::new();
    let mut owners = Vec::new();

    let abc_start = MULTI_AB;
    let ac_start = abc_start + MULTI_ABC;
    for i in 0..ac_start + MULTI_AC {
        let (b, c) = (i < ac_start, i >= abc_start);
        let id = world.add_entity(A(a_value(i)));
        if b {
            world.add_component(id, B(b_value(i)));
        }
        if c {
            world.add_component(id, C(c_value(i)));
        }
        owners.push(Owner { id, i, b, c });
    }
    for i in 0..3 {
        world.add_entity(B(b_value(ac_start + MULTI_AC + i)));
    }

    if layout == Layout::Grouped {
        world.group::<(A, B)>().group::<(A, C)>();
    }

    for k in 0..JOINS {
        let ab = &mut owners[spread(k, JOINS, MULTI_AB) as usize];
        world.add_component(ab.id, C(c_value(ab.i)));
        ab.c = true;

        let ac = &mut owners[(ac_start + spread(k, JOINS, MULTI_AC)) as usize];
        world.add_component(ac.id, B(b_value(ac.i)));
        ac.b = true;
    }

    if layout == Layout::Grouped {
        world.regroup();
    }

    (world, owners)
}

/// The multiple-group world, for `(&A, &B)`: a full match of AB, a projection of the union.
///
/// - grouped: AB's range then the union's, which A's and B's wrap points cut in several
///   segments. Candidates: `MULTI_AB + MULTI_ABC + JOINS`.
/// - ordinary: B captains, A is looked up. Candidates: `MULTI_AB + MULTI_ABC + JOINS + 3`.
///
/// Yields: `MULTI_AB + MULTI_ABC + JOINS` (74,009).
pub fn multiple_pairs(layout: Layout) -> Fixture {
    let (world, owners) = multiple_world(layout);
    let candidates = match layout {
        Layout::Grouped => MULTI_AB + MULTI_ABC + JOINS,
        Layout::Ordinary => MULTI_AB + MULTI_ABC + JOINS + 3,
    };
    // Neither walk claims an exact length: one has follow-up segments, the other looks up A.
    let mut fixture = Fixture::new(world, candidates, false);
    fixture.model(&owners, false);

    fixture
}

/// The multiple-group world, for `(&A, &B, &C)`: a full match of the union.
///
/// - grouped: the union's range, which A's and B's wrap points cut in several segments.
///   Candidates: `MULTI_ABC + 2 * JOINS`.
/// - ordinary: C captains, A and B are looked up. Candidates: `MULTI_ABC + MULTI_AC + JOINS`.
///
/// Yields: `MULTI_ABC + 2 * JOINS` (38,009).
pub fn multiple_triples(layout: Layout) -> Fixture {
    let (world, owners) = multiple_world(layout);
    let candidates = match layout {
        Layout::Grouped => MULTI_ABC + 2 * JOINS,
        Layout::Ordinary => MULTI_ABC + MULTI_AC + JOINS,
    };
    // Neither walk claims an exact length: one has follow-up segments, the other has lookups.
    let mut fixture = Fixture::new(world, candidates, false);
    fixture.model(&owners, true);

    fixture
}

/// `N` AB pairs, 3 A-only and 3 B-only entities, optionally grouped. Then `CHURN` pairs spread
/// over the second half lose their B, storage-local removals leaving A in place,
/// and `CHURN` new pairs are added.
fn churned_pairs(layout: Layout, regroup: bool, candidates: u32, exact: bool) -> Fixture {
    let mut fixture = Fixture::new(World::new(), candidates, exact);
    let pairs: Vec<_> = (0..N)
        .map(|i| fixture.world.add_entity((A(a_value(i)), B(b_value(i)))))
        .collect();
    for i in N..N + 3 {
        fixture.add_lone_a(i);
        fixture.world.add_entity(B(b_value(i)));
    }

    if layout == Layout::Grouped {
        fixture.world.group::<(A, B)>();
    }

    let half = N / 2;
    let mut loses_b = vec![false; N as usize];
    for k in 0..CHURN {
        loses_b[(half + spread(k, CHURN, N - half)) as usize] = true;
    }
    for (i, (&id, loses_b)) in (0..).zip(pairs.iter().zip(loses_b)) {
        if loses_b {
            fixture.world.delete_component::<B>(id);
            fixture.unmatched_a.push((id, a_value(i)));
        } else {
            fixture.matches.push(Match {
                id,
                a: a_value(i),
                b: b_value(i),
                c: None,
            });
        }
    }

    for i in N + 3..N + 3 + CHURN {
        fixture.add_pair(i);
    }

    if regroup {
        fixture.world.regroup();
    }

    fixture
}

/// `churned_pairs`, grouped and left damaged. The walks are:
///
/// - aligned, up to the first position the removals touched in B's range
/// - the rest of B's range, B being the shorter member, A looked up
/// - A's and B's unevaluated regions, the new pairs, B's walk covered by A's
///
/// Candidates: `N - CHURN + 2 * CHURN`. Yields: `N`.
pub fn damaged() -> Fixture {
    churned_pairs(Layout::Grouped, false, N + CHURN, false)
}

/// `churned_pairs`, grouped then regrouped: a single aligned segment again.
///
/// Candidates: `N`, exact. Yields: `N`.
pub fn repaired() -> Fixture {
    churned_pairs(Layout::Grouped, true, N, true)
}

/// `churned_pairs` without any group. B captains, A is looked up.
///
/// Candidates: `N + 3`. Yields: `N`.
pub fn churned_ordinary() -> Fixture {
    churned_pairs(Layout::Ordinary, false, N + 3, false)
}

/// `HEAD` AB pairs, `N` entities also owning a C, then 3 A-only and 3 B-only entities.
///
/// - grouped as `(A, B)` and `(A, B, C)`: AB's tiny range comes first, then the ABC range
///   as one large follow-up. The lone components are ungrouped and not walked.
///   Candidates: `HEAD + N`.
/// - ordinary: A or B captains, they're the same size. Candidates: `HEAD + N + 3`.
///
/// Yields: `HEAD + N`. The first `HEAD` matches are the AB pairs.
#[cfg(feature = "parallel")]
pub fn tail(layout: Layout) -> Fixture {
    let candidates = match layout {
        Layout::Grouped => HEAD + N,
        Layout::Ordinary => HEAD + N + 3,
    };
    // Neither walk claims an exact length: one has a follow-up segment, the other looks up B.
    let mut fixture = Fixture::new(World::new(), candidates, false);
    for i in 0..HEAD {
        fixture.add_pair(i);
    }
    for i in HEAD..HEAD + N {
        let (a, b) = (a_value(i), b_value(i));
        let id = fixture.world.add_entity((A(a), B(b), C(c_value(i))));
        fixture.matches.push(Match { id, a, b, c: None });
    }
    for i in HEAD + N..HEAD + N + 3 {
        fixture.add_lone_a(i);
        fixture.world.add_entity(B(b_value(i)));
    }

    if layout == Layout::Grouped {
        fixture.world.group::<(A, B)>().group::<(A, B, C)>();
    }

    fixture
}

/// A change a lifecycle case makes to an entity before regrouping.
#[derive(Clone, Copy)]
pub enum Change {
    /// Deletes its A.
    RemoveA,
    /// Deletes its A, then gives it this one.
    ReaddA(u32),
    RemoveB,
    ReaddB(u32),
    /// Gives it this C.
    AddC(u32),
    RemoveC,
    RemoveH,
    /// Deletes the entity.
    Delete,
}

/// What a lifecycle case does to its fixture before regrouping.
///
/// It's made once from a fixture's model, then applied to fresh copies of the same fixture:
/// building a fixture always creates the same entities.
pub struct Churn {
    /// Changes to the fixture's matches. Every deletion happens before any addition, like
    /// deleting a batch of components then restoring it.
    pub changes: Vec<(EntityId, Change)>,
    /// Value indices of the pairs added after the changes.
    pub added: Range<u32>,
}

impl Churn {
    /// Makes `change(k, m)` to the `k`th of `matches`.
    pub fn each(matches: &[Match], change: impl Fn(usize, &Match) -> Change) -> Churn {
        Churn {
            changes: matches
                .iter()
                .enumerate()
                .map(|(k, m)| (m.id, change(k, m)))
                .collect(),
            added: 0..0,
        }
    }

    /// Also adds the pairs of value indices `added`, after the changes.
    pub fn adding(self, added: Range<u32>) -> Churn {
        Churn { added, ..self }
    }

    /// Returns how many entities the churn changes or adds.
    pub fn len(&self) -> usize {
        self.changes.len() + self.added.len()
    }

    /// Makes the changes to `world`, then adds the pairs. `added` gets each new pair's id and
    /// value index.
    pub fn apply(&self, world: &mut World, mut added: impl FnMut(EntityId, u32)) {
        for &(id, change) in &self.changes {
            match change {
                Change::RemoveA | Change::ReaddA(_) => world.delete_component::<A>(id),
                Change::RemoveB | Change::ReaddB(_) => world.delete_component::<B>(id),
                Change::RemoveC => world.delete_component::<C>(id),
                Change::RemoveH => world.delete_component::<H>(id),
                Change::Delete => {
                    world.delete_entity(id);
                }
                Change::AddC(_) => {}
            }
        }

        for &(id, change) in &self.changes {
            match change {
                Change::ReaddA(a) => world.add_component(id, A(a)),
                Change::ReaddB(b) => world.add_component(id, B(b)),
                Change::AddC(c) => world.add_component(id, C(c)),
                _ => {}
            }
        }

        for i in self.added.clone() {
            let id = world.add_entity((A(a_value(i)), B(b_value(i))));
            added(id, i);
        }
    }
}

/// `N` entities owning A, B and an H, grouped as `(A, B, H)`.
///
/// For `(&A, &B, &H)`, candidates: `N`, exact. Yields: `N`.
pub fn heavy() -> Fixture {
    let mut fixture = Fixture::new(World::new(), N, true);
    for i in 0..N {
        let (a, b) = (a_value(i), b_value(i));
        let id = fixture.world.add_entity((A(a), B(b), H(h_value(a))));
        fixture.matches.push(Match { id, a, b, c: None });
    }
    fixture.world.group::<(A, B, H)>();

    fixture
}

/// The multiple-group world, for both `(&A, &B)` and `(&A, &B, &C)`.
///
/// The model holds every entity owning A and B, with its C when it owns one: the pair query
/// yields them all, the triple query the ones owning a C. Candidates are left to the checks.
pub fn overlap() -> Fixture {
    let (world, owners) = multiple_world(Layout::Grouped);
    let mut fixture = Fixture::new(world, 0, false);
    for owner in &owners {
        if owner.b {
            fixture.matches.push(Match {
                id: owner.id,
                a: a_value(owner.i),
                b: b_value(owner.i),
                c: owner.c.then(|| c_value(owner.i)),
            });
        } else {
            fixture.unmatched_a.push((owner.id, a_value(owner.i)));
        }
    }

    fixture
}

/// Promotions to `overlap`'s union and demotions from it, each spread over the entities that can
/// take it:
///
/// - `CHURN` pairs owning only A and B gain a C
/// - `2 * CHURN` union entities lose their C or their B, alternately
///
/// Only B and C lose components, so A's range of the union stays fully trusted while holding
/// entities that have to leave it, and other entities have to join it from outside.
pub fn overlap_churn(fixture: &Fixture) -> Churn {
    let (union, pairs): (Vec<&Match>, Vec<&Match>) =
        fixture.matches.iter().partition(|m| m.c.is_some());

    let mut changes = Vec::new();
    for k in 0..CHURN {
        let m = pairs[spread(k, CHURN, pairs.len() as u32) as usize];
        changes.push((m.id, Change::AddC(c_value(m.a))));
    }
    for k in 0..2 * CHURN {
        let m = union[spread(k, 2 * CHURN, union.len() as u32) as usize];
        let change = if k % 2 == 0 {
            Change::RemoveC
        } else {
            Change::RemoveB
        };
        changes.push((m.id, change));
    }

    Churn {
        changes,
        added: 0..0,
    }
}
