//! The pending case: a workload leaving twelve grouped storages pending at once, more than the
//! eight the executor keeps inline.
//!
//! Twelve components `P0` to `P11` form six groups of two: `(P0, P1)`, `(P2, P3)`... The workload:
//!
//! 1. Two systems write `P0` to `P3` and `P4` to `P7`: the eight storages the executor keeps
//!    inline are pending.
//! 2. Two more systems share `(P8, P9)` and `(P10, P11)`, one writing `P8` and `P10`, the other `P9`
//!    and `P11`: those two sets each get two writes from two systems, and their four storages are
//!    the ninth to the twelfth pending, past the inline ones.
//! 3. `read_first` reads `P0` and `P1`, the `(P0, P1)` regroup runs before it: a partial clear.
//! 4. `write_p1` writes `P1` again, `(P0, P1)` is pending again.
//! 5. `read_second` reads `P8` and `P9`, the `(P8, P9)` regroup runs before it: a partial clear of
//!    storages past the inline ones.
//! 6. The workload ends with five sets left pending, their regroups run.
//!
//! Timed runs only borrow, so they time the executor's bookkeeping and clean regroups. When
//! `DAMAGE` is set, for the checks, every write removes the target's component and gives it back,
//! so each obligation has work to do and a missing regroup leaves its set out of order.

use shipyard::scheduler::info::RegroupTrigger;
use shipyard::{
    AddComponent, Component, EntityId, IntoIter, Remove, View, ViewMut, Workload, World, track,
};
use std::sync::atomic::{AtomicBool, Ordering};

macro_rules! components {
    ($($name:ident),*) => {
        $(
            pub struct $name(#[allow(dead_code)] pub u32);
            impl Component for $name {
                type Tracking = track::Untracked;
            }
        )*
    };
}

components!(P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11);

/// Entities owning every component.
const ENTITIES: u32 = 16;
/// The workload's name.
pub const NAME: &str = "twelve_pending";

/// Whether writes damage the sets, for the checks only.
static DAMAGE: AtomicBool = AtomicBool::new(false);
/// Whether `read_first` and `read_second` last saw their set as a single aligned segment.
static FIRST_ALIGNED: AtomicBool = AtomicBool::new(false);
static SECOND_ALIGNED: AtomicBool = AtomicBool::new(false);

/// Removes `target`'s component and gives it back, when `DAMAGE` is set.
fn write<T: Component + Send + Sync>(view: &mut ViewMut<T>, target: EntityId) {
    if DAMAGE.load(Ordering::Relaxed) {
        let component = view.remove(target).unwrap();
        view.add_component_unchecked(target, component);
    }
}

/// Returns `true` when `(a, b)` walks a single aligned segment holding every entity.
fn aligned<A: Component + Send + Sync, B: Component + Send + Sync>(
    a: &View<A>,
    b: &View<B>,
) -> bool {
    let n = ENTITIES as usize;
    (a, b).iter().size_hint() == (n, Some(n))
}

/// A world declaring the six groups, every set clean.
pub struct Pending {
    pub world: World,
    target: EntityId,
}

impl Pending {
    pub fn new() -> Pending {
        let mut world = World::builder()
            .with_group::<(P0, P1)>()
            .with_group::<(P2, P3)>()
            .with_group::<(P4, P5)>()
            .with_group::<(P6, P7)>()
            .with_group::<(P8, P9)>()
            .with_group::<(P10, P11)>()
            .build();

        let mut target = None;
        for i in 0..ENTITIES {
            let id = world.add_entity((P0(i), P1(i), P2(i), P3(i), P4(i), P5(i)));
            world.add_component(id, (P6(i), P7(i), P8(i), P9(i), P10(i), P11(i)));
            target.get_or_insert(id);
        }
        world.regroup();

        let pending = Pending {
            world,
            target: target.unwrap(),
        };
        pending.workload().add_to_world(&pending.world).unwrap();

        pending
    }

    fn workload(&self) -> Workload {
        let target = self.target;

        Workload::new(NAME)
            .with_system(
                move |mut p0: ViewMut<P0>,
                      mut p1: ViewMut<P1>,
                      mut p2: ViewMut<P2>,
                      mut p3: ViewMut<P3>| {
                    write(&mut p0, target);
                    write(&mut p1, target);
                    write(&mut p2, target);
                    write(&mut p3, target);
                },
            )
            .with_system(
                move |mut p4: ViewMut<P4>,
                      mut p5: ViewMut<P5>,
                      mut p6: ViewMut<P6>,
                      mut p7: ViewMut<P7>| {
                    write(&mut p4, target);
                    write(&mut p5, target);
                    write(&mut p6, target);
                    write(&mut p7, target);
                },
            )
            .with_system(move |mut p8: ViewMut<P8>, mut p10: ViewMut<P10>| {
                write(&mut p8, target);
                write(&mut p10, target);
            })
            .with_system(move |mut p9: ViewMut<P9>, mut p11: ViewMut<P11>| {
                write(&mut p9, target);
                write(&mut p11, target);
            })
            .with_system(|p0: View<P0>, p1: View<P1>| {
                FIRST_ALIGNED.store(aligned(&p0, &p1), Ordering::Relaxed);
            })
            .with_system(move |mut p1: ViewMut<P1>| write(&mut p1, target))
            .with_system(|p8: View<P8>, p9: View<P9>| {
                SECOND_ALIGNED.store(aligned(&p8, &p9), Ordering::Relaxed);
            })
    }

    /// Runs the workload once, the call the benchmark times.
    pub fn run(&self) {
        self.world.run_workload(NAME).unwrap();
    }

    /// Asserts that the workload plans six entry regroups and seven pending ones, then runs it
    /// with writes damaging the sets: both readers must see their set regrouped, and every set
    /// must be regrouped once the workload ends.
    pub fn check(&self) {
        let info = self.world.workloads_info().0[NAME].clone();
        let regroups: Vec<_> = info
            .batches_info
            .iter()
            .flat_map(|batch| &batch.regroups)
            .collect();
        let count = |trigger| regroups.iter().filter(|r| r.trigger == trigger).count();
        assert_eq!(count(RegroupTrigger::Entry), 6, "{NAME}: entry regroups");
        assert_eq!(
            count(RegroupTrigger::Pending),
            7,
            "{NAME}: pending regroups"
        );

        // A damaged set isn't aligned until it's regrouped, so the checks below can tell.
        let target = self.target;
        DAMAGE.store(true, Ordering::Relaxed);
        self.world.run(|mut p0: ViewMut<P0>| write(&mut p0, target));
        let first = self.world.run(|a: View<P0>, b: View<P1>| aligned(&a, &b));
        assert!(!first, "{NAME}: damage not visible");
        self.world.regroup_set::<P0>().unwrap();
        self.check_aligned();

        FIRST_ALIGNED.store(false, Ordering::Relaxed);
        SECOND_ALIGNED.store(false, Ordering::Relaxed);
        DAMAGE.store(true, Ordering::Relaxed);
        self.run();
        DAMAGE.store(false, Ordering::Relaxed);

        assert!(FIRST_ALIGNED.load(Ordering::Relaxed), "{NAME}: first read");
        assert!(
            SECOND_ALIGNED.load(Ordering::Relaxed),
            "{NAME}: second read"
        );
        self.check_aligned();

        // Timed runs only borrow, their regroups find every set clean.
        self.run();
        self.check_aligned();
    }

    /// Asserts that every set walks a single aligned segment.
    fn check_aligned(&self) {
        let world = &self.world;
        let sets = [
            world.run(|a: View<P0>, b: View<P1>| aligned(&a, &b)),
            world.run(|a: View<P2>, b: View<P3>| aligned(&a, &b)),
            world.run(|a: View<P4>, b: View<P5>| aligned(&a, &b)),
            world.run(|a: View<P6>, b: View<P7>| aligned(&a, &b)),
            world.run(|a: View<P8>, b: View<P9>| aligned(&a, &b)),
            world.run(|a: View<P10>, b: View<P11>| aligned(&a, &b)),
        ];
        assert!(sets.iter().all(|&aligned| aligned), "{NAME}: sets {sets:?}");
    }
}
