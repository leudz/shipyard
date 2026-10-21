//! Small worlds, the workloads timed on them, and the checks they pass before any timing.
//!
//! Every world holds the same entities, created in the same order, so an entity id means the
//! same entity in each of them. Grouped worlds declare `(A, B)`, sometimes `(C, D)` too, and start
//! regrouped, clean.

use crate::iteration_fixtures::{A, B, C, a_value, b_value, c_value};
use shipyard::scheduler::info::{RegroupTrigger, WorkloadInfo};
use shipyard::scheduler::{ScheduledWorkload, SystemModificator};
use shipyard::{Component, EntityId, Get, IntoIter, View, ViewMut, Workload, World, track};
use std::hint::black_box;
use std::sync::atomic::{AtomicU32, Ordering};

/// The second member of the `(C, D)` group.
///
/// Nothing reads it, it's only there to be grouped with C.
pub struct D(#[allow(dead_code)] pub u32);
impl Component for D {
    type Tracking = track::Untracked;
}

/// AB pairs in the small worlds, each also owning a C and a D.
pub const SMALL: u32 = 103;
/// What the writers give to the target's A.
pub const WRITTEN: u32 = 0xC0FF_EE00;
/// What `LAST_READ` holds before a checked run, no system reads it.
const UNREAD: u32 = u32::MAX;

/// The last value a reader read, so reads can't be optimized away and checks can see them.
pub static LAST_READ: AtomicU32 = AtomicU32::new(UNREAD);

/// The groups a small world declares.
#[derive(Clone, Copy, PartialEq)]
pub enum Groups {
    /// `(A, B)`, regrouped once populated.
    Ab,
    /// `(A, B)` and `(C, D)`, two connected sets, regrouped once populated.
    AbCd,
    /// None at all.
    None,
}

/// A small world and the entities its workloads touch.
pub struct Small {
    pub world: World,
    pub groups: Groups,
    /// The entity the systems read and write, the first pair.
    pub target: EntityId,
    /// Sorted ids of the AB pairs.
    pub pairs: Vec<EntityId>,
}

/// Returns `SMALL` pairs owning A, B, C and D, then 3 A-only and 3 B-only entities.
pub fn small(groups: Groups) -> Small {
    let mut world = match groups {
        Groups::Ab => World::builder().with_group::<(A, B)>().build(),
        Groups::AbCd => World::builder()
            .with_group::<(A, B)>()
            .with_group::<(C, D)>()
            .build(),
        Groups::None => World::new(),
    };

    let mut pairs: Vec<EntityId> = (0..SMALL)
        .map(|i| world.add_entity((A(a_value(i)), B(b_value(i)), C(c_value(i)), D(i))))
        .collect();
    for i in SMALL..SMALL + 3 {
        world.add_entity(A(a_value(i)));
        world.add_entity(B(b_value(i)));
    }
    world.regroup();

    let target = pairs[0];
    pairs.sort_unstable();

    Small {
        world,
        groups,
        target,
        pairs,
    }
}

impl Small {
    /// Asserts that `(&A, &B)` yields exactly the pairs, as a single aligned segment when grouped.
    pub fn check_members(&self, case: &str) {
        let (a, b) = self.world.borrow::<(View<A>, View<B>)>().unwrap();

        if self.groups != Groups::None {
            let n = self.pairs.len();
            let hint = (&a, &b).iter().size_hint();
            assert_eq!(hint, (n, Some(n)), "{case}: not a single aligned segment");
        }

        let mut ids: Vec<_> = (&a, &b).iter().ids().collect();
        ids.sort_unstable();
        assert!(ids == self.pairs, "{case}: wrong members");
    }

    /// Returns the target's A.
    fn target_a(&self) -> u32 {
        let a = self.world.borrow::<View<A>>().unwrap();
        a.get(self.target).unwrap().0
    }

    /// Gives the target back the A it was created with.
    fn reset_target(&self) {
        let mut a = self.world.borrow::<ViewMut<A>>().unwrap();
        (&mut a).get(self.target).unwrap().0 = a_value(0);
    }
}

/// A workload the benchmark times on small worlds.
#[derive(Clone, Copy)]
pub enum Case {
    /// Reads the target's A.
    ReadOnly,
    /// Writes the target's A, then reads it.
    ValueWriter,
    /// Writes the target's A, nothing runs after it.
    FinalWriter,
    /// `ValueWriter` with its writer's condition always false.
    SkippedWriter,
    /// Reads the target's C, a storage outside any group.
    UngroupedReader,
    /// Reads the target's A and C, only A is grouped.
    CrossReader,
    /// `CrossReader`'s workload in a world where C has its own group, two sets to regroup.
    TwoSetReader,
}

pub const CASES: [Case; 7] = [
    Case::ReadOnly,
    Case::ValueWriter,
    Case::FinalWriter,
    Case::SkippedWriter,
    Case::UngroupedReader,
    Case::CrossReader,
    Case::TwoSetReader,
];

impl Case {
    pub fn name(self) -> &'static str {
        match self {
            Case::ReadOnly => "read_only",
            Case::ValueWriter => "value_writer",
            Case::FinalWriter => "final_writer",
            Case::SkippedWriter => "skipped_writer",
            Case::UngroupedReader => "ungrouped_reader",
            Case::CrossReader => "cross_reader",
            Case::TwoSetReader => "two_set_reader",
        }
    }

    /// Returns the groups of the world the registered and standalone variants run on.
    fn groups(self) -> Groups {
        match self {
            Case::TwoSetReader => Groups::AbCd,
            _ => Groups::Ab,
        }
    }

    /// Returns the variants timed for the case.
    pub fn variants(self) -> &'static [Variant] {
        match self {
            // Its group-free variant would be `CrossReader`'s.
            Case::TwoSetReader => &[Variant::Registered, Variant::Standalone],
            _ => &[Variant::Registered, Variant::Standalone, Variant::GroupFree],
        }
    }

    /// Returns the case's workload, the same for every variant.
    pub fn workload(self, target: EntityId) -> Workload {
        let workload = Workload::new(self.name());

        match self {
            Case::ReadOnly => workload.with_system(read_a(target)),
            Case::ValueWriter => workload
                .with_system(write_a(target))
                .with_system(read_a(target)),
            Case::FinalWriter => workload.with_system(write_a(target)),
            Case::SkippedWriter => workload
                .with_system(write_a(target).run_if(|| false))
                .with_system(read_a(target)),
            Case::UngroupedReader => workload.with_system(read_c(target)),
            Case::CrossReader | Case::TwoSetReader => workload.with_system(read_a_c(target)),
        }
    }

    /// Returns the triggers of the regroups the workload prepares once added to a grouped world.
    fn registered_triggers(self) -> &'static [RegroupTrigger] {
        use RegroupTrigger::{Entry, Pending};

        match self {
            Case::ReadOnly | Case::CrossReader => &[Entry],
            Case::ValueWriter | Case::FinalWriter | Case::SkippedWriter => &[Entry, Pending],
            Case::UngroupedReader => &[],
            Case::TwoSetReader => &[Entry, Entry],
        }
    }

    /// Returns the target's A after a run.
    fn expected_a(self) -> u32 {
        match self {
            Case::ValueWriter | Case::FinalWriter => WRITTEN,
            _ => a_value(0),
        }
    }

    /// Returns what the workload's reader reads, `UNREAD` without one.
    fn expected_read(self) -> u32 {
        match self {
            Case::ReadOnly | Case::SkippedWriter => a_value(0),
            Case::ValueWriter => WRITTEN,
            Case::FinalWriter => UNREAD,
            Case::UngroupedReader => c_value(0),
            Case::CrossReader | Case::TwoSetReader => a_value(0) ^ c_value(0),
        }
    }
}

fn read_a(target: EntityId) -> impl Fn(View<A>) + Send + Sync + 'static {
    move |a: View<A>| LAST_READ.store(a.get(target).unwrap().0, Ordering::Relaxed)
}

fn read_c(target: EntityId) -> impl Fn(View<C>) + Send + Sync + 'static {
    move |c: View<C>| LAST_READ.store(c.get(target).unwrap().0, Ordering::Relaxed)
}

fn read_a_c(target: EntityId) -> impl Fn(View<A>, View<C>) + Send + Sync + 'static {
    move |a: View<A>, c: View<C>| {
        let value = a.get(target).unwrap().0 ^ c.get(target).unwrap().0;
        LAST_READ.store(value, Ordering::Relaxed);
    }
}

/// Assigns the same value each time, so repeated runs stay valid and never change membership.
fn write_a(target: EntityId) -> impl Fn(ViewMut<A>) + Send + Sync + 'static {
    move |mut a: ViewMut<A>| (&mut a).get(target).unwrap().0 = black_box(WRITTEN)
}

/// How a case's workload runs.
#[derive(Clone, Copy)]
pub enum Variant {
    /// Added to a grouped world, with its automatic regroups.
    Registered,
    /// Built with `Workload::build` and run on a grouped world, without any regroup.
    Standalone,
    /// Added to a world without groups.
    GroupFree,
}

impl Variant {
    pub fn name(self) -> &'static str {
        match self {
            Variant::Registered => "registered",
            Variant::Standalone => "standalone",
            Variant::GroupFree => "group_free",
        }
    }
}

/// A case's workload, ready to run on its own small world.
pub struct Prepared {
    pub small: Small,
    pub case: Case,
    /// Only set for the standalone variant, the others are run by name.
    pub standalone: Option<ScheduledWorkload>,
}

impl Prepared {
    /// Runs the workload once, through the same call the benchmark times.
    pub fn run(&self) {
        let world = &self.small.world;

        match &self.standalone {
            Some(scheduled) => scheduled.run_with_world(world).unwrap(),
            None => world.run_workload(self.case.name()).unwrap(),
        }
    }
}

/// Builds `case`'s world and workload for `variant`, checking the regroups it prepared.
pub fn prepare(case: Case, variant: Variant) -> Prepared {
    let groups = match variant {
        Variant::Registered | Variant::Standalone => case.groups(),
        Variant::GroupFree => Groups::None,
    };
    let small = small(groups);
    let label = format!("{}/{}", case.name(), variant.name());
    let workload = case.workload(small.target);

    let (standalone, info, expected) = match variant {
        Variant::Standalone => {
            let (scheduled, info) = workload.build().unwrap();
            (Some(scheduled), info, &[][..])
        }
        Variant::Registered | Variant::GroupFree => {
            workload.add_to_world(&small.world).unwrap();
            let info = small.world.workloads_info().0[case.name()].clone();
            let expected = match variant {
                Variant::Registered => case.registered_triggers(),
                _ => &[],
            };
            (None, info, expected)
        }
    };
    check_regroups(&label, &info, expected);

    Prepared {
        small,
        case,
        standalone,
    }
}

/// Asserts that `info` regroups two-storage sets with exactly the `expected` triggers, in order.
pub fn check_regroups(label: &str, info: &WorkloadInfo, expected: &[RegroupTrigger]) {
    let regroups: Vec<_> = info
        .batches_info
        .iter()
        .flat_map(|batch| &batch.regroups)
        .collect();

    let triggers: Vec<_> = regroups.iter().map(|regroup| regroup.trigger).collect();
    assert_eq!(triggers, expected, "{label}: unexpected regroups");
    for regroup in regroups {
        assert_eq!(regroup.borrow.len(), 2, "{label}: not a two-storage set");
    }
}

/// Runs `prepared` a few times, asserting after each run that its systems did what they should
/// and the pairs are still where they were.
pub fn check_runs(prepared: &Prepared, label: &str) {
    let small = &prepared.small;
    let case = prepared.case;

    for _ in 0..3 {
        small.reset_target();
        LAST_READ.store(UNREAD, Ordering::Relaxed);

        prepared.run();

        assert_eq!(small.target_a(), case.expected_a(), "{label}: wrong A");
        let read = LAST_READ.load(Ordering::Relaxed);
        assert_eq!(read, case.expected_read(), "{label}: wrong read");
        small.check_members(label);
    }
}
