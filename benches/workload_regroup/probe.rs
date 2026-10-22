//! The probe cases: entry regroups of two disjoint sets sharing a batch, where at least one of the
//! regroups left after the calling thread's step has work to do, so the parallel job is spawned
//! whether or not the executor checked their cleanliness first.
//!
//! - `disjoint_dirty`: a reader of A and C. Its first batch holds the AB and CD regroups only, the
//!   calling thread takes the first one, the other one is left. Both sets have work to do.
//! - `clean_then_dirty`: the same reader plus an unrelated system reading E. Its first batch holds
//!   that system, taken by the calling thread, then both regroups, left in order. The first one
//!   left is clean, the second one has work to do.
//!
//! Damage is tail-local and small: B (or D) removed from the `TAIL` last pairs of a set and given
//! back, the same value. A persistent world gets it before each timed run, outside the timer, and
//! each run's regroups repair it.

use crate::fixtures::D;
use crate::iteration_fixtures::{A, B, C, a_value, b_value, c_value};
use shipyard::advanced::StorageId;
use shipyard::scheduler::info::{RegroupTrigger, WorkloadInfo};
use shipyard::sparse_set::SparseSet;
use shipyard::{
    AddComponent, Component, Delete, EntityId, Get, IntoIter, View, ViewMut, Workload, World, track,
};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// A component outside any group, read by the unrelated system.
pub struct E;
impl Component for E {
    type Tracking = track::Untracked;
}

/// Pairs in each set.
const PAIRS: u32 = 103;
/// Pairs at the logical end of a set losing a member and getting it back.
const TAIL: usize = 3;

/// What the reader last read.
static LAST_READ: AtomicU32 = AtomicU32::new(0);
/// Whether the unrelated system last ran on the thread that ran the workload.
static UNRELATED_ON_CALLER: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Set on the thread running the workload, for the unrelated system to tell where it runs.
    static CALLER: Cell<bool> = const { Cell::new(false) };
}

#[derive(Clone, Copy)]
pub enum Probe {
    DisjointDirty,
    CleanThenDirty,
}

pub const PROBES: [Probe; 2] = [Probe::DisjointDirty, Probe::CleanThenDirty];

/// A set of the world.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Set {
    Ab,
    Cd,
}

impl Probe {
    pub fn name(self) -> &'static str {
        match self {
            Probe::DisjointDirty => "disjoint_dirty",
            Probe::CleanThenDirty => "clean_then_dirty",
        }
    }
}

/// A probe case's world, with its workload added, and what it damages before each run.
pub struct Prepared {
    pub probe: Probe,
    pub world: World,
    /// The tail pairs, with their B and D.
    tail: Vec<(EntityId, u32, u32)>,
    /// The sets damaged before each run.
    damaged: Vec<Set>,
    /// Sorted ids of every pair.
    pairs: Vec<EntityId>,
    /// The physical layout the first damage left, every damage has to leave the same.
    damaged_layout: Option<u64>,
}

impl Prepared {
    /// Builds the world, adds the workload and checks the batch holding the two regroups.
    pub fn new(probe: Probe) -> Prepared {
        let mut world = World::builder()
            .with_group::<(A, B)>()
            .with_group::<(C, D)>()
            .build();
        let mut pairs: Vec<EntityId> = (0..PAIRS)
            .map(|i| {
                let components = (A(a_value(i)), B(b_value(i)), C(c_value(i)), D(i), E);
                world.add_entity(components)
            })
            .collect();
        world.regroup();

        let tail = pairs[pairs.len() - TAIL..]
            .iter()
            .zip(PAIRS - TAIL as u32..)
            .map(|(&id, i)| (id, b_value(i), i))
            .collect();
        let target = pairs[0];
        pairs.sort_unstable();

        let workload = Workload::new(probe.name()).with_system(read_a_c(target));
        let workload = match probe {
            Probe::DisjointDirty => workload,
            Probe::CleanThenDirty => workload.with_system(unrelated),
        };
        workload.add_to_world(&world).unwrap();

        let info = world.workloads_info().0[probe.name()].clone();
        let order = regroup_order(probe, &info);
        let damaged = match probe {
            Probe::DisjointDirty => order.to_vec(),
            // Only the second regroup left has work to do.
            Probe::CleanThenDirty => vec![order[1]],
        };

        Prepared {
            probe,
            world,
            tail,
            damaged,
            pairs,
            damaged_layout: None,
        }
    }

    /// Damages the sets the case damages, checking they need a regroup, the others don't, and
    /// that the layout is the one every damage left so far.
    pub fn damage(&mut self) {
        for &set in &self.damaged {
            match set {
                Set::Ab => {
                    let mut b = self.world.borrow::<ViewMut<B>>().unwrap();
                    for &(id, _, _) in &self.tail {
                        b.delete(id);
                    }
                    for &(id, value, _) in &self.tail {
                        b.add_component_unchecked(id, B(value));
                    }
                }
                Set::Cd => {
                    let mut d = self.world.borrow::<ViewMut<D>>().unwrap();
                    for &(id, _, _) in &self.tail {
                        d.delete(id);
                    }
                    for &(id, _, value) in &self.tail {
                        d.add_component_unchecked(id, D(value));
                    }
                }
            }
        }

        let name = self.probe.name();
        for set in [Set::Ab, Set::Cd] {
            let aligned = self.aligned(set);
            assert_eq!(
                aligned,
                !self.damaged.contains(&set),
                "{name}: {set:?} damage"
            );
        }

        let layout = self.layout();
        assert_eq!(
            *self.damaged_layout.get_or_insert(layout),
            layout,
            "{name}: other damage"
        );
    }

    /// Runs the workload once, marking the current thread as the caller.
    pub fn run(&self) {
        CALLER.set(true);
        self.world.run_workload(self.probe.name()).unwrap();
        CALLER.set(false);
    }

    /// Asserts that the last run regrouped both sets, read the target, and that the unrelated
    /// system, if any, was the calling thread's step.
    pub fn check_run(&self) {
        let name = self.probe.name();

        for set in [Set::Ab, Set::Cd] {
            assert!(self.aligned(set), "{name}: {set:?} not regrouped");
        }
        let read = LAST_READ.load(Ordering::Relaxed);
        assert_eq!(read, a_value(0) ^ c_value(0), "{name}: wrong read");
        if let Probe::CleanThenDirty = self.probe {
            let on_caller = UNRELATED_ON_CALLER.load(Ordering::Relaxed);
            assert!(
                on_caller,
                "{name}: unrelated system wasn't the calling thread's step"
            );
        }
    }

    /// Returns the fingerprint of the damaged layout, the same on two builds when they repair the
    /// same layout.
    pub fn damaged_layout(&self) -> u64 {
        self.damaged_layout.expect("damage ran")
    }

    /// Returns `true` when the set's pairs form a single aligned segment holding every pair.
    fn aligned(&self, set: Set) -> bool {
        let n = self.pairs.len();
        let (hint, mut ids): (_, Vec<_>) = match set {
            Set::Ab => {
                let (a, b) = self.world.borrow::<(View<A>, View<B>)>().unwrap();
                ((&a, &b).iter().size_hint(), (&a, &b).iter().ids().collect())
            }
            Set::Cd => {
                let (c, d) = self.world.borrow::<(View<C>, View<D>)>().unwrap();
                ((&c, &d).iter().size_hint(), (&c, &d).iter().ids().collect())
            }
        };
        ids.sort_unstable();
        assert!(ids == self.pairs, "{}: wrong members", self.probe.name());

        hint == (n, Some(n))
    }

    /// Returns a hash of the physical order of A, B, C and D, then of the walks of both sets,
    /// FNV-1a style.
    ///
    /// Giving back a removed member puts it back in the same row, the walks are what tell the
    /// damaged sets apart.
    fn layout(&self) -> u64 {
        let mut hash = 0xCBF2_9CE4_8422_2325;
        hash_dense::<A>(&self.world, &mut hash);
        hash_dense::<B>(&self.world, &mut hash);
        hash_dense::<C>(&self.world, &mut hash);
        hash_dense::<D>(&self.world, &mut hash);

        let (a, b, c, d) = self
            .world
            .borrow::<(View<A>, View<B>, View<C>, View<D>)>()
            .unwrap();
        for (lower, upper) in [(&a, &b).iter().size_hint(), (&c, &d).iter().size_hint()] {
            mix(&mut hash, lower as u64);
            mix(&mut hash, upper.map_or(u64::MAX, |upper| upper as u64));
        }

        hash
    }
}

fn hash_dense<T: Component + Send + Sync>(world: &World, hash: &mut u64) {
    let view = world.borrow::<View<T>>().unwrap();
    for index in 0..view.len() {
        mix(hash, view.id_at(index).unwrap().inner());
    }
}

fn mix(hash: &mut u64, value: u64) {
    *hash = (*hash ^ value).wrapping_mul(0x0100_0000_01B3);
}

/// Asserts that the first batch holds both entry regroups, with the unrelated system when the
/// case has one, and the reader waits in the second. Returns the sets in regroup order.
///
/// Within a batch, steps keep the planner's order: the system the batch started with, then the
/// regroups added to it. `check_run` confirms the system is the calling thread's step.
fn regroup_order(probe: Probe, info: &WorkloadInfo) -> [Set; 2] {
    let name = probe.name();
    let batches = &info.batches_info;
    assert_eq!(batches.len(), 2, "{name}: unexpected batches");

    let first = &batches[0];
    let unrelated: Vec<_> = first.systems().collect();
    match probe {
        Probe::DisjointDirty => assert!(unrelated.is_empty(), "{name}: unexpected system"),
        Probe::CleanThenDirty => {
            assert!(
                first.systems.0.is_none(),
                "{name}: system on the world's thread"
            );
            assert_eq!(first.systems.1.len(), 1, "{name}: unrelated system missing");
        }
    }
    assert!(
        batches[1].regroups.is_empty(),
        "{name}: regroup next to the reader"
    );
    assert_eq!(batches[1].systems().count(), 1, "{name}: reader missing");

    let triggers: Vec<_> = first.regroups.iter().map(|r| r.trigger).collect();
    assert_eq!(
        triggers,
        [RegroupTrigger::Entry; 2],
        "{name}: unexpected regroups"
    );

    let d = StorageId::of::<SparseSet<D>>();
    let set = |index: usize| {
        let borrows_d = first.regroups[index]
            .borrow
            .iter()
            .any(|type_info| type_info.storage_id == d);
        if borrows_d { Set::Cd } else { Set::Ab }
    };
    let order = [set(0), set(1)];
    assert_ne!(order[0], order[1], "{name}: same set twice");

    order
}

fn read_a_c(target: EntityId) -> impl Fn(View<A>, View<C>) + Send + Sync + 'static {
    move |a: View<A>, c: View<C>| {
        let value = a.get(target).unwrap().0 ^ c.get(target).unwrap().0;
        LAST_READ.store(value, Ordering::Relaxed);
    }
}

fn unrelated(e: View<E>) {
    std::hint::black_box(e.len());
    UNRELATED_ON_CALLER.store(CALLER.get(), Ordering::Relaxed);
}
