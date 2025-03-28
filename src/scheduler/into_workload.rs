use crate::scheduler::info::DedupedLabels;
use crate::scheduler::label::{SequentialLabel, WorkloadLabel};
use crate::scheduler::system::WorkloadSystem;
use crate::scheduler::workload::Workload;
use crate::scheduler::{AsLabel, IntoWorkloadSystem, WorkloadModificator};
use crate::type_id::TypeId;
use alloc::vec::Vec;
use core::any::{type_name, Any};
use core::sync::atomic::{AtomicU32, Ordering};
// macro not module
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec;

static WORKLOAD_ID: AtomicU32 = AtomicU32::new(1);
fn unique_id() -> u32 {
    WORKLOAD_ID.fetch_add(1, Ordering::Relaxed)
}

/// Converts to a collection of systems.
///
/// To modify the workload execution see [WorkloadModificator](crate::WorkloadModificator).
pub trait IntoWorkload<Views, R> {
    /// Converts to a collection of systems.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{Component, EntitiesViewMut, IntoIter, IntoWorkload, View, ViewMut, Workload, World};
    ///
    /// #[derive(Component)]
    /// struct Health(f32);
    /// #[derive(Component)]
    /// struct Fat(f32);
    ///
    /// fn initial_population(
    ///     mut entities: EntitiesViewMut,
    ///     mut healths: ViewMut<Health>,
    ///     mut fats: ViewMut<Fat>,
    /// ) {
    ///     entities.bulk_add_entity(
    ///         (&mut healths, &mut fats),
    ///         (0..100).map(|_| (Health(100.0), Fat(0.0))),
    ///     );
    /// }
    ///
    /// fn reproduction(
    ///     mut fats: ViewMut<Fat>,
    ///     mut healths: ViewMut<Health>,
    ///     mut entities: EntitiesViewMut,
    /// ) {
    ///     let count = (&healths, &fats)
    ///         .iter()
    ///         .filter(|(health, fat)| health.0 > 40.0 && fat.0 > 20.0)
    ///         .count();
    ///
    ///     entities.bulk_add_entity(
    ///         (&mut healths, &mut fats),
    ///         (0..count).map(|_| (Health(100.0), Fat(0.0))),
    ///     );
    /// }
    ///
    /// fn meal(mut fats: ViewMut<Fat>) {
    ///     for mut fat in (&mut fats).iter() {
    ///         fat.0 += 3.0;
    ///     }
    /// }
    ///
    /// fn age(mut healths: ViewMut<Health>) {
    ///     (&mut healths).iter().for_each(|mut health| {
    ///         health.0 -= 4.0;
    ///     });
    /// }
    ///
    /// fn life() -> Workload {
    ///     (meal, age).into_workload()
    /// }
    ///
    /// let world = World::new();
    /// world.run(initial_population);
    ///
    /// world.add_workload(life);
    ///
    /// for day in 0..100 {
    ///     if day % 6 == 0 {
    ///         world.run(reproduction);
    ///     }
    ///     world.run_default_workload().unwrap();
    /// }
    ///
    /// // we've got some new pigs
    /// assert_eq!(world.borrow::<View<Health>>().unwrap().len(), 900);
    /// ```
    fn into_workload(self) -> Workload;
    /// Converts to a collection of systems.  
    /// All systems will run one after the other. Does not propagate into nested [`Workload`] but they will run sequentially between them.
    ///
    /// Not different than [`into_workload`](IntoWorkload::into_workload) for a single system.
    ///
    /// ### Panics
    ///
    /// - If two identical systems are present in the workload. This is a limitation with the current expressivity of `before`/`after`.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{IntoWorkload, Workload};
    ///
    /// fn sys1() {}
    /// fn sys2() {}
    /// fn sys3() {}
    /// fn sys4() {}
    /// fn workload1() -> Workload {
    ///     (sys1, sys2).into_workload()
    /// }
    ///
    /// (workload1, sys3, sys4).into_sequential_workload();
    /// ```
    ///
    /// In this example `sys1` and `sys2` can run in parallel but always before `sys3`.  
    /// `sys3` and `sys4` run sequentially.
    fn into_sequential_workload(self) -> Workload;
}

impl IntoWorkload<Workload, Workload> for Workload {
    fn into_workload(self) -> Workload {
        self
    }
    fn into_sequential_workload(mut self) -> Workload {
        let mut system_names = DedupedLabels::with_capacity(self.systems.len());

        for system in &self.systems {
            if !system_names.add(system.type_id.as_label()) {
                panic!("{:?} appears twice in this workload. `into_sequential_workload` cannot currently handle this case.", system.display_name);
            }
        }

        for index in 0..self.systems.len() {
            if let Some(next_system) = self.systems.get(index + 1) {
                let tag = SequentialLabel(next_system.type_id.as_label());
                self.systems[index].before_all.add(tag);
            }
        }

        self
    }
}

impl<Views, R, Sys> IntoWorkload<Views, R> for Sys
where
    Sys: IntoWorkloadSystem<Views, R> + 'static,
    R: 'static,
{
    fn into_workload(self) -> Workload {
        if TypeId::of::<R>() == TypeId::of::<Workload>() {
            let workload: Box<dyn Any> = Box::new(self.call());
            let mut workload = *workload.downcast::<Workload>().unwrap();

            let label = WorkloadLabel {
                type_id: TypeId::of::<Sys>(),
                name: type_name::<Sys>().as_label(),
            };

            workload = workload.tag(label.clone());
            workload.name = Box::new(label);

            workload
        } else if TypeId::of::<R>() == TypeId::of::<WorkloadSystem>() {
            let system: Box<dyn Any> = Box::new(self.call());
            let system = *system.downcast::<WorkloadSystem>().unwrap();

            system.into_workload()
        } else {
            let system = self.into_workload_system().unwrap();

            let unique_id = unique_id();

            let name = Box::new(WorkloadLabel {
                type_id: TypeId(unique_id as u128),
                name: unique_id.to_string().as_label(),
            });

            Workload {
                name: name.clone(),
                tags: vec![name],
                systems: vec![system],
                run_if: None,
                before_all: DedupedLabels::new(),
                after_all: DedupedLabels::new(),
                overwritten_name: false,
                require_before: DedupedLabels::new(),
                require_after: DedupedLabels::new(),
                barriers: Vec::new(),
            }
        }
    }

    fn into_sequential_workload(self) -> Workload {
        let workload = self.into_workload();

        if TypeId::of::<R>() == TypeId::of::<Workload>() {
            workload.into_sequential_workload()
        } else {
            workload
        }
    }
}

macro_rules! impl_into_workload {
    ($(($type: ident, $borrow: ident, $return: ident, $index: tt))+) => {
        impl<$($type, $borrow, $return),+> IntoWorkload<($($borrow,)+), ($($return,)+)> for ($($type,)+)
        where
            $(
                $type: IntoWorkload<$borrow, $return> + 'static,
            )+
        {
            fn into_workload(self) -> Workload {
                let unique_id = unique_id();

                let name = Box::new(WorkloadLabel {
                    type_id: TypeId(unique_id as u128),
                    name: unique_id.to_string().as_label(),
                });

                let mut workload = Workload {
                    tags: vec![name.clone()],
                    name,
                    systems: Vec::new(),
                    run_if: None,
                    before_all: DedupedLabels::new(),
                    after_all: DedupedLabels::new(),
                    overwritten_name: false,
                    require_before: DedupedLabels::new(),
                    require_after: DedupedLabels::new(),
                    barriers: Vec::new(),
                };

                $(
                    let w = self.$index.into_workload();
                    workload = workload.merge(w);
                )+

                workload
            }

            #[track_caller]
            fn into_sequential_workload(self) -> Workload {
                let unique_id = unique_id();

                let name = Box::new(WorkloadLabel {
                    type_id: TypeId(unique_id as u128),
                    name: unique_id.to_string().as_label(),
                });

                let mut workload = Workload {
                    tags: vec![name.clone()],
                    name,
                    systems: Vec::new(),
                    run_if: None,
                    before_all: DedupedLabels::new(),
                    after_all: DedupedLabels::new(),
                    overwritten_name: false,
                    require_before: DedupedLabels::new(),
                    require_after: DedupedLabels::new(),
                    barriers: Vec::new(),
                };

                let mut sequential_tags = Vec::new();

                let mut workloads = ($({
                    let mut w = self.$index.into_workload();

                    let tag = SequentialLabel(w.name.clone());
                    w = w.tag(tag.clone());

                    sequential_tags.push(tag);

                    w
                },)+);

                $(
                    if let Some(sequential_tag) = sequential_tags.get($index + 1) {
                        workloads.$index = workloads.$index.before_all(sequential_tag.clone());
                    }

                    workload = workload.merge(workloads.$index);
                )+

                let mut system_names = DedupedLabels::with_capacity(workload.systems.len());

                for system in &workload.systems {
                    if !system_names.add(system.type_id.as_label()) {
                        panic!("{:?} appears twice in this workload. `into_sequential_workload` cannot currently handle this case.", system.display_name);
                    }
                }

                workload
            }
        }
    };
}

macro_rules! into_workload {
    ($(($type: ident, $borrow: ident, $return: ident, $index: tt))*;($type1: ident, $borrow1: ident, $return1: ident, $index1: tt) $(($queue_type: ident, $queue_borrow: ident, $queue_return: ident, $queue_index: tt))*) => {
        impl_into_workload![$(($type, $borrow, $return, $index))*];
        into_workload![$(($type, $borrow, $return, $index))* ($type1, $borrow1, $return1, $index1); $(($queue_type, $queue_borrow, $queue_return, $queue_index))*];
    };
    ($(($type: ident, $borrow: ident, $return: ident, $index: tt))*;) => {
        impl_into_workload![$(($type, $borrow, $return, $index))*];
    }
}

#[cfg(not(feature = "extended_tuple"))]
into_workload![
    (ViewA, A, Ra, 0); (ViewB, B, Rb, 1) (ViewC, C, Rc, 2) (ViewD, D, Rd, 3) (ViewE, E, Re, 4) (ViewF, F, Rf, 5) (ViewG, G, Rg, 6) (ViewH, H, Rh, 7) (ViewI, I, Ri, 8) (ViewJ, J, Rj, 9)
    (ViewK, K, Rk, 10) (ViewL, L, Rl, 11) (ViewM, M, Rm, 12) (ViewN, N, Rn, 13) (ViewO, O, Ro, 14) (ViewP, P, Rp, 15) (ViewQ, Q, Rq, 16) (ViewR, R, Rr, 17) (ViewS, S, Rs, 18) (ViewT, T, Rt, 19)
];
#[cfg(feature = "extended_tuple")]
into_workload![
    (ViewA, A, Ra, 0); (ViewB, B, Rb, 1) (ViewC, C, Rc, 2) (ViewD, D, Rd, 3) (ViewE, E, Re, 4) (ViewF, F, Rf, 5) (ViewG, G, Rg, 6) (ViewH, H, Rh, 7) (ViewI, I, Ri, 8) (ViewJ, J, Rj, 9)
    (ViewK, K, Rk, 10) (ViewL, L, Rl, 11) (ViewM, M, Rm, 12) (ViewN, N, Rn, 13) (ViewO, O, Ro, 14) (ViewP, P, Rp, 15) (ViewQ, Q, Rq, 16) (ViewR, R, Rr, 17) (ViewS, S, Rs, 18) (ViewT, T, Rt, 19)
    (ViewU, U, Ru, 20) (ViewV, V, Rv, 21) (ViewW, W, Rw, 22) (ViewX, X, Rx, 23) (ViewY, Y, Ry, 24) (ViewZ, Z, Rz, 25) (ViewAA, AA, Raa, 26) (ViewBB, BB, Rbb, 27) (ViewCC, CC, Rcc, 28) (ViewDD, DD, Rdd, 29)
    (ViewEE, EE, Ree, 30) (ViewFF, FF, Rff, 31)
];
