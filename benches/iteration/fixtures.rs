//! Worlds built through the public API, with an independent model of what each query yields.
//!
//! Every fixture records the entities its query should yield and the values they were given,
//! straight from construction. Checks compare iteration against that model, never against
//! another iteration.

use shipyard::{track, Component, EntityId, Get, ViewMut, World};

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
    let mut fixture = Fixture::new(World::new(), count, true);
    for i in 0..count {
        fixture.add_pair(i);
    }
    for i in count..count + 3 {
        fixture.add_lone_a(i);
        fixture.world.add_entity(B(b_value(i)));
    }
    fixture.world.group::<(A, B)>();
    fixture
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
