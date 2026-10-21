//! Randomized derived queries over grouped worlds, through the public API only.
//!
//! A smaller sibling of the library's `grouping_differential`, which covers the other query
//! shapes. Each seed mutates a world and a model in lockstep, then derived views are compared
//! with what the model holds. Expected items never come from iterating a storage.\
//! A failure prints the seed and the step, `replay(seed, steps)` rebuilds that world.

use shipyard::*;
use std::collections::BTreeMap;

const SEEDS: u64 = 24;
const STEPS: usize = 48;

/// What the writes add to every component they reach.
const DELTA: u32 = 1_000_000;

#[derive(Component, Debug, PartialEq)]
struct A(u32);

#[derive(Component, Debug, PartialEq)]
struct B(u32);

#[derive(Component, Debug, PartialEq)]
struct C(u32);

#[derive(Borrow, IntoIter)]
struct PairView<'v> {
    a: View<'v, A>,
    b: View<'v, B>,
}

#[derive(Borrow, IntoIter)]
struct PairTuple<'v>(View<'v, A>, View<'v, B>);

#[derive(Borrow, IntoIter)]
struct PairMutView<'v> {
    a: ViewMut<'v, A>,
    b: ViewMut<'v, B>,
}

/// Values of an entity's `A`, `B` and `C`, `None` when it doesn't own one.
type Row = [Option<u32>; 3];

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

/// A world and the model it has to match.
struct Scenario {
    rng: Rng,
    world: World,
    model: BTreeMap<EntityId, Row>,
    last_value: u32,
}

impl Scenario {
    /// Returns the scenario of `seed`: a quarter of them never group, a quarter group AB and AC,
    /// the others only AB.
    fn new(seed: u64) -> Scenario {
        let world = match seed % 4 {
            0 => World::new(),
            2 => World::builder()
                .with_group::<(A, B)>()
                .with_group::<(A, C)>()
                .build(),
            _ => World::builder().with_group::<(A, B)>().build(),
        };

        Scenario {
            rng: Rng(seed),
            world,
            model: BTreeMap::new(),
            last_value: 0,
        }
    }

    /// Returns the scenario of `seed` after its first `steps` steps.
    fn replay(seed: u64, steps: usize) -> Scenario {
        let mut scenario = Scenario::new(seed);
        for _ in 0..steps {
            scenario.step();
        }

        scenario
    }

    fn new_value(&mut self) -> u32 {
        self.last_value += 1;
        self.last_value
    }

    /// Applies a random mutation and returns its name, for failure messages.
    fn step(&mut self) -> &'static str {
        let slot = self.rng.below(3);
        let roll = self.rng.below(100);

        let picked = if self.model.is_empty() {
            None
        } else {
            let position = self.rng.below(self.model.len());
            self.model.keys().nth(position).copied()
        };
        let Some(id) = picked.filter(|_| roll >= 35) else {
            self.add_entity();
            return "add entity";
        };

        match roll {
            35..55 => {
                let value = self.new_value();
                match slot {
                    0 => self.world.add_component(id, A(value)),
                    1 => self.world.add_component(id, B(value)),
                    _ => self.world.add_component(id, C(value)),
                }
                self.model.get_mut(&id).unwrap()[slot] = Some(value);
                "add component"
            }
            55..72 => {
                match slot {
                    0 => self.world.delete_component::<A>(id),
                    1 => self.world.delete_component::<B>(id),
                    _ => self.world.delete_component::<C>(id),
                }
                self.model.get_mut(&id).unwrap()[slot] = None;
                "delete component"
            }
            72..84 => {
                self.world.delete_entity(id);
                self.model.remove(&id);
                "delete entity"
            }
            _ => {
                self.world.regroup();
                "regroup"
            }
        }
    }

    /// Adds an entity owning each component with a 2 in 3 chance.
    fn add_entity(&mut self) {
        let mut row = [None; 3];
        for value in &mut row {
            if self.rng.below(3) != 0 {
                *value = Some(self.new_value());
            }
        }

        let id = self.world.add_entity(());
        if let Some(a) = row[0] {
            self.world.add_component(id, A(a));
        }
        if let Some(b) = row[1] {
            self.world.add_component(id, B(b));
        }
        if let Some(c) = row[2] {
            self.world.add_component(id, C(c));
        }

        self.model.insert(id, row);
    }

    /// Returns the model's items, sorted by id. `item` returns `None` for rows it doesn't match.
    fn expect<V>(&self, item: impl Fn(&Row) -> Option<V>) -> Vec<(EntityId, V)> {
        self.model
            .iter()
            .filter_map(|(&id, row)| Some((id, item(row)?)))
            .collect()
    }
}

/// Asserts that `items`, in any order, are exactly `expected`, which is sorted by id.
///
/// Duplicates are reported as such, sorting can't hide them.
fn assert_items<V: Clone + std::fmt::Debug + PartialEq>(
    mut items: Vec<(EntityId, V)>,
    expected: &[(EntityId, V)],
    context: &str,
) {
    items.sort_by_key(|(id, _)| *id);

    if let Some(pair) = items.windows(2).find(|pair| pair[0].0 == pair[1].0) {
        panic!("{:?} yielded twice, {context}", pair[0].0);
    }

    assert_eq!(items, expected, "{context}");
}

fn pair(row: &Row) -> Option<(u32, u32)> {
    Some((row[0]?, row[1]?))
}

fn triple(row: &Row) -> Option<(u32, u32, u32)> {
    Some((row[0]?, row[1]?, row[2]?))
}

/// Compares the derived views of `scenario` with its model.
fn check(scenario: &Scenario, rng: &mut Rng, context: &str) {
    let (pairs, tuple, c) = scenario
        .world
        .borrow::<(PairView, PairTuple, View<C>)>()
        .unwrap();
    let expected = scenario.expect(pair);

    // `next` gives the order, `fold` has to follow it, even after a `next` prefix.
    let next: Vec<_> = (&pairs)
        .iter()
        .with_id()
        .map(|(id, item)| (id, (item.a.0, item.b.0)))
        .collect();
    assert_items(next.clone(), &expected, &format!("named, {context}"));

    let prefix = rng.below(next.len() + 2);
    let mut iter = (&pairs).iter().with_id();
    let mut items: Vec<_> = (&mut iter)
        .take(prefix)
        .map(|(id, item)| (id, (item.a.0, item.b.0)))
        .collect();
    items = iter.fold(items, |mut items, (id, item)| {
        items.push((id, (item.a.0, item.b.0)));
        items
    });
    assert_eq!(items, next, "named, {prefix} next then fold, {context}");

    let tuple_items = (&tuple)
        .iter()
        .with_id()
        .fold(Vec::new(), |mut items, (id, (a, b))| {
            items.push((id, (a.0, b.0)));
            items
        });
    assert_items(tuple_items, &expected, &format!("tuple, {context}"));

    let mut nested = Vec::new();
    (&pairs, &c)
        .iter()
        .with_id()
        .for_each(|(id, (item, c))| nested.push((id, (item.a.0, item.b.0, c.0))));
    assert_items(
        nested,
        &scenario.expect(triple),
        &format!("nested, {context}"),
    );

    #[cfg(feature = "parallel")]
    {
        use rayon::iter::ParallelIterator;

        let parallel: Vec<_> = (&pairs)
            .par_iter()
            .with_id()
            .map(|(id, item)| (id, (item.a.0, item.b.0)))
            .collect();
        assert_items(parallel, &expected, &format!("parallel named, {context}"));
    }
}

/// Asserts that derived views agree with the model after every step of every seed.
#[test]
fn reads() {
    for seed in 0..SEEDS {
        let mut scenario = Scenario::new(seed);
        let mut rng = Rng(!seed);

        for step in 0..STEPS {
            let op = scenario.step();
            check(
                &scenario,
                &mut rng,
                &format!("seed {seed}, step {step} ({op})"),
            );
        }
    }
}

/// Asserts that every `A` and `B` holds the model's value, plus `delta` for the AB pairs.
fn assert_written(scenario: &Scenario, delta: u32, context: &str) {
    let (a, b) = scenario.world.borrow::<(View<A>, View<B>)>().unwrap();

    for (&id, row) in &scenario.model {
        let added = if pair(row).is_some() { delta } else { 0 };

        assert_eq!(
            a.get(id).ok().map(|a| a.0),
            row[0].map(|v| v + added),
            "{context}"
        );
        assert_eq!(
            b.get(id).ok().map(|b| b.0),
            row[1].map(|v| v + added),
            "{context}"
        );
    }
}

/// Asserts that a mutable derived view writes every AB pair exactly once, sequentially and in
/// parallel, each on its own replayed world.
#[test]
fn writes() {
    for seed in 0..SEEDS {
        let context = format!("seed {seed}, {STEPS} steps");

        let scenario = Scenario::replay(seed, STEPS);
        {
            let mut pairs = scenario.world.borrow::<PairMutView>().unwrap();
            (&mut pairs).iter().for_each(|item| {
                item.a.0 += DELTA;
                item.b.0 += DELTA;
            });
        }
        assert_written(&scenario, DELTA, &format!("for_each, {context}"));

        #[cfg(feature = "parallel")]
        {
            use rayon::iter::ParallelIterator;

            let scenario = Scenario::replay(seed, STEPS);
            {
                let mut pairs = scenario.world.borrow::<PairMutView>().unwrap();
                (&mut pairs).par_iter().for_each(|item| {
                    item.a.0 += DELTA;
                    item.b.0 += DELTA;
                });
            }
            assert_written(&scenario, DELTA, &format!("parallel, {context}"));
        }
    }
}
