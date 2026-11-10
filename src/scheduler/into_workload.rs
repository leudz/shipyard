use crate::scheduler::info::DedupedLabels;
use crate::scheduler::label::{SequentialLabel, WorkloadLabel};
use crate::scheduler::system::WorkloadSystem;
use crate::scheduler::workload::Workload;
use crate::scheduler::{AsLabel, IntoWorkloadSystem, WorkloadModificator};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;
use core::any::{Any, type_name};
// macro not module
use alloc::vec;

/// Turns a collection of systems into a [`Workload`](crate::scheduler::Workload)
///
/// To modify the workload execution see [WorkloadModificator](crate::scheduler::WorkloadModificator).\
/// To run the systems one after the other see [`IntoSequentialWorkload`].
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
}

/// Turns a collection of systems into a [`Workload`](crate::scheduler::Workload) running them one after the other.
///
/// It's implemented for tuples, single systems and functions returning a [`Workload`].\
/// [`Workload`] itself doesn't implement it, put it in a tuple to order it with other systems.
///
/// ### Migration
///
/// `into_sequential_workload` used to be a method of [`IntoWorkload`], import this trait to call it.\
/// Calling it on a [`Workload`] used to compile but didn't order anything, a [`Workload`] has
/// already lost the order of its children. That call doesn't compile anymore.
pub trait IntoSequentialWorkload<Views, R> {
    /// Converts to a collection of systems running one after the other.
    ///
    /// Each element of the tuple runs after the previous one.
    /// A nested [`Workload`], or a function returning one, is a single element:
    /// its systems keep their own ordering and can run in parallel with each other.
    ///
    /// Not different than [`into_workload`](IntoWorkload::into_workload) for a single system
    /// or a function returning a [`Workload`].
    ///
    /// ### Panics
    ///
    /// - If two identical systems are present in the workload. This is a limitation with the current expressivity of `before`/`after`.
    ///
    /// ### Example:
    /// ```
    /// use shipyard::{IntoSequentialWorkload, IntoWorkload, Workload};
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
            // Single system, sys.into_workload()

            let system = self.into_workload_system().unwrap();

            let name = Box::new(WorkloadLabel {
                type_id: TypeId::of::<Sys>(),
                name: type_name::<Sys>().as_label(),
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
}

impl<Views, R, Sys> IntoSequentialWorkload<Views, R> for Sys
where
    Sys: IntoWorkloadSystem<Views, R> + 'static,
    R: 'static,
{
    fn into_sequential_workload(self) -> Workload {
        // A single system has nothing to order.
        // A function returning a workload is a single child, its systems keep their own ordering.
        IntoWorkload::<Views, R>::into_workload(self)
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
                let name = Box::new(WorkloadLabel {
                    type_id: TypeId::of::<($($type,)+)>(),
                    name: type_name::<($($type,)+)>().as_label(),
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
        }

        impl<$($type, $borrow, $return),+> IntoSequentialWorkload<($($borrow,)+), ($($return,)+)> for ($($type,)+)
        where
            $(
                $type: IntoWorkload<$borrow, $return> + 'static,
            )+
        {
            #[track_caller]
            fn into_sequential_workload(self) -> Workload {
                let name = Box::new(WorkloadLabel {
                    type_id: TypeId::of::<($($type,)+)>(),
                    name: type_name::<($($type,)+)>().as_label(),
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

                let mut children = vec![$(self.$index.into_workload()),+];

                // We link each child to the next one with systems.
                // An empty child has no system to carry a link, skipping it keeps its neighbors linked.
                let conversion = SequentialLabel::new_conversion();
                let nonempty: Vec<usize> = (0..children.len())
                    .filter(|&child| !children[child].systems.is_empty())
                    .collect();
                for pair in nonempty.windows(2) {
                    let (previous, next) = (pair[0], pair[1]);
                    let label = SequentialLabel::new(conversion, next);

                    children[next].tags.push(Box::new(label));
                    children[previous].before_all.add(label);
                }

                for child in children {
                    workload = workload.merge(child);
                }

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
