//! The structural cases: a writer removing B from `CHURN` pairs of `aligned`'s world and giving
//! it back, then a reader folding every pair.
//!
//! The pairs are picked like the iteration benchmark's `lifecycle/regroup` cases: spread over the
//! group's range for `scattered`, at its logical end for `tail`. Building `aligned` always creates
//! the same entities, so pairs picked from one fixture name the same entities in a fresh one.
//! B is given back with the same value, so the fixture's model holds after any number of runs.

use crate::fixtures::check_regroups;
use crate::iteration_fixtures::{self, A, B, CHURN, Fixture};
use shipyard::scheduler::ScheduledWorkload;
use shipyard::scheduler::info::RegroupTrigger;
use shipyard::{AddComponent, Delete, EntityId, IntoIter, View, ViewMut, Workload, World};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// How many times a writer ran.
static WRITES: AtomicU32 = AtomicU32::new(0);
/// The sum the reader last folded, `Fixture::pair_sum`'s formula.
static LAST_SUM: AtomicU32 = AtomicU32::new(0);
/// The lower bound of the reader's last walk, every pair when it was a single aligned segment.
static LAST_LOWER: AtomicUsize = AtomicUsize::new(0);

/// Where the writer damages the group.
#[derive(Clone, Copy)]
pub enum Damage {
    Scattered,
    Tail,
}

pub const DAMAGES: [Damage; 2] = [Damage::Scattered, Damage::Tail];

impl Damage {
    pub fn name(self) -> &'static str {
        match self {
            Damage::Scattered => "scattered",
            Damage::Tail => "tail",
        }
    }
}

/// A structural case, with the model of its world and the standalone control's workloads.
pub struct Structural {
    pub damage: Damage,
    /// The pairs the writer changes, with their B.
    pairs: Vec<(EntityId, u32)>,
    /// `aligned`'s model, the same before and after a run.
    model: Fixture,
    /// The control's writer and reader, each built on its own so a regroup can go in between.
    pub writer: ScheduledWorkload,
    pub reader: ScheduledWorkload,
}

impl Structural {
    pub fn new(damage: Damage) -> Structural {
        let model = iteration_fixtures::aligned();
        let matches = match damage {
            Damage::Scattered => model.spread_matches(CHURN),
            Damage::Tail => model.tail_matches(CHURN),
        };
        let pairs: Vec<_> = matches.iter().map(|m| (m.id, m.b)).collect();

        let (writer, _) = Workload::new("writer")
            .with_system(write_b(pairs.clone()))
            .build()
            .unwrap();
        let (reader, _) = Workload::new("reader")
            .with_system(read_pairs)
            .build()
            .unwrap();

        Structural {
            damage,
            pairs,
            model,
            writer,
            reader,
        }
    }

    /// Returns a fresh `aligned` world, regrouped, clean.
    pub fn world(&self) -> World {
        iteration_fixtures::aligned().world
    }

    /// Returns a fresh `aligned` world with the writer and reader added as a single workload.
    pub fn registered_world(&self) -> World {
        let world = self.world();
        Workload::new(self.damage.name())
            .with_system(write_b(self.pairs.clone()))
            .with_system(read_pairs)
            .add_to_world(&world)
            .unwrap();

        world
    }

    /// Runs the workload `registered_world` added.
    pub fn run_registered(&self, world: &World) {
        world.run_workload(self.damage.name()).unwrap();
    }

    /// Does what the registered workload does, with explicit regroups and two dispatches.
    pub fn run_standalone(&self, world: &World) {
        // The registered workload's entry regroup.
        world.regroup_set::<A>().unwrap();
        self.writer.run_with_world(world).unwrap();
        // Its pending regroup, the reader borrows B again.
        world.regroup_set::<A>().unwrap();
        self.reader.run_with_world(world).unwrap();
    }

    /// Asserts that the writer damages a fresh world, and that both variants leave it as the
    /// model says after their reader saw it regrouped.
    pub fn check(&self) {
        let name = self.damage.name();

        // Without a regroup, the writer leaves the pairs outside a single aligned segment.
        // Every timed run starts from a fresh world, so each of them has this damage to repair.
        let world = self.world();
        let writes = WRITES.load(Ordering::Relaxed);
        self.writer.run_with_world(&world).unwrap();
        assert_eq!(
            WRITES.load(Ordering::Relaxed),
            writes + 1,
            "{name}: writer didn't run"
        );
        let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
        let n = self.model.yielded();
        assert_ne!(
            (&a, &b).iter().size_hint(),
            (n, Some(n)),
            "{name}: no damage"
        );
        drop((a, b));

        let world = self.registered_world();
        let info = world.workloads_info().0[name].clone();
        let expected = [RegroupTrigger::Entry, RegroupTrigger::Pending];
        check_regroups(name, &info, &expected);
        self.check_run(&format!("{name}/registered"), &world, Self::run_registered);

        let world = self.world();
        self.check_run(&format!("{name}/standalone"), &world, Self::run_standalone);
    }

    /// Runs `run` once on `world`, asserting that the writer ran, the reader saw the pairs
    /// regrouped and the world holds the model.
    fn check_run(&self, label: &str, world: &World, run: fn(&Structural, &World)) {
        let n = self.model.yielded();
        let writes = WRITES.load(Ordering::Relaxed);
        LAST_SUM.store(0, Ordering::Relaxed);
        LAST_LOWER.store(0, Ordering::Relaxed);

        run(self, world);

        assert_eq!(
            WRITES.load(Ordering::Relaxed),
            writes + 1,
            "{label}: writer didn't run"
        );
        let lower = LAST_LOWER.load(Ordering::Relaxed);
        assert_eq!(lower, n, "{label}: reader didn't walk a regrouped world");
        let sum = LAST_SUM.load(Ordering::Relaxed);
        assert_eq!(
            sum,
            self.model.pair_sum(),
            "{label}: reader got wrong values"
        );

        let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
        let hint = (&a, &b).iter().size_hint();
        assert_eq!(hint, (n, Some(n)), "{label}: not a single aligned segment");
        let mut ids: Vec<_> = (&a, &b).iter().ids().collect();
        ids.sort_unstable();
        assert!(ids == self.model.ids(), "{label}: wrong members");
    }
}

/// Removes B from every one of `pairs` and gives it back, like the lifecycle cases' churn:
/// every removal before any addition.
fn write_b(pairs: Vec<(EntityId, u32)>) -> impl Fn(ViewMut<B>) + Send + Sync + 'static {
    move |mut b: ViewMut<B>| {
        for &(id, _) in &pairs {
            b.delete(id);
        }
        for &(id, value) in &pairs {
            b.add_component_unchecked(id, B(value));
        }

        WRITES.fetch_add(1, Ordering::Relaxed);
    }
}

fn read_pairs(a: View<A>, b: View<B>) {
    let iter = (&a, &b).iter();
    LAST_LOWER.store(iter.size_hint().0, Ordering::Relaxed);

    let sum = iter.fold(0u32, |sum, (a, b)| sum.wrapping_add(a.0 ^ (b.0 << 3)));
    LAST_SUM.store(sum, Ordering::Relaxed);
}
