use crate::info::Requirements;
use crate::scheduler::{IntoWorkloadSystem, Label, WorkloadBuilder};
use crate::type_id::TypeId;
use crate::view::AllStoragesView;
use crate::{AsLabel, WorkloadSystem};
use alloc::boxed::Box;
use alloc::vec::Vec;
// macro not module
use alloc::vec;

impl crate::World {
    /// Creates a new workload and store it in the [`World`](crate::World).
    pub fn add_workload<Views, R, W, F: Fn() -> W + 'static>(&self, workload: F)
    where
        W: IntoWorkload<Views, R>,
    {
        let w = workload().into_workload();

        WorkloadBuilder {
            work_units: w.work_units,
            name: Box::new(TypeId::of::<F>()),
            skip_if: Vec::new(),
            before: w.before,
            after: w.after,
        }
        .add_to_world(self)
        .unwrap();
    }
}

/// A collection of system.
pub struct Workload {
    #[allow(unused)]
    pub(super) name: Option<Box<dyn Label>>,
    pub(super) work_units: Vec<super::builder::WorkUnit>,
    #[allow(unused)]
    pub(super) skip_if: Vec<Box<dyn Fn(AllStoragesView<'_>) -> bool + Send + Sync + 'static>>,
    pub(super) before: Requirements,
    pub(super) after: Requirements,
}

impl Workload {
    /// Creates a new empty [`WorkloadBuilder`].
    ///
    /// [`WorkloadBuilder`]: crate::WorkloadBuilder
    pub fn builder<L: Label>(label: L) -> WorkloadBuilder {
        WorkloadBuilder::new(label)
    }
}

/// Converts to a collection of systems.
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
    ///     for fat in (&mut fats).iter() {
    ///         fat.0 += 3.0;
    ///     }
    /// }
    ///
    /// fn age(mut healths: ViewMut<Health>) {
    ///     (&mut healths).iter().for_each(|health| {
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
    ///     world.run_default().unwrap();
    /// }
    ///
    /// // we've got some new pigs
    /// assert_eq!(world.borrow::<View<Health>>().unwrap().len(), 900);
    /// ```
    fn into_workload(self) -> Workload;
    /// When building a workload, this system or workload will be placed before all invocation of the other system or workload.
    fn before_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem;
    /// When building a workload, this system or workload will be placed after all invocation of the other system or workload.
    fn after_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem;
}

impl IntoWorkload<(), ()> for Workload {
    fn into_workload(self) -> Workload {
        self
    }

    fn before_all<T>(mut self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.before.add(other.as_label());

        WorkloadSystem::Workload(self)
    }

    fn after_all<T>(mut self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.after.add(other.as_label());

        WorkloadSystem::Workload(self)
    }
}

impl<Views, R, Sys> IntoWorkload<Views, R> for Sys
where
    Sys: IntoWorkloadSystem<Views, R>,
{
    fn into_workload(self) -> Workload {
        Workload {
            name: None,
            work_units: vec![self.into_workload_system().unwrap().into()],
            skip_if: Vec::new(),
            before: Requirements::new(),
            after: Requirements::new(),
        }
    }

    fn before_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.into_workload().before_all(other)
    }

    fn after_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.into_workload().after_all(other)
    }
}

impl IntoWorkload<(), ()> for WorkloadBuilder {
    fn into_workload(self) -> Workload {
        Workload {
            name: Some(self.name),
            work_units: self.work_units,
            skip_if: self.skip_if,
            before: self.before,
            after: self.after,
        }
    }

    fn before_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.into_workload().before_all(other)
    }

    fn after_all<T>(self, other: impl AsLabel<T>) -> WorkloadSystem {
        self.into_workload().after_all(other)
    }
}

macro_rules! impl_system {
    ($(($type: ident, $borrow: ident, $return: ident, $index: tt))+) => {
        impl<$($type, $borrow, $return),+> IntoWorkload<($($borrow,)+), ($($return,)+)> for ($($type,)+)
        where
            $(
                $type: IntoWorkload<$borrow, $return>,
            )+
        {
            fn into_workload(self) -> Workload {
                let mut workload = Workload {
                    name: None,
                    work_units: Vec::new(),
                    skip_if: Vec::new(),
                    before: Requirements::new(),
                    after: Requirements::new(),
                };

                $(
                    let w = self.$index.into_workload();
                    workload.work_units.extend(w.work_units);
                )+

                workload
            }

            fn before_all<Label>(self, other: impl AsLabel<Label>) -> WorkloadSystem {
                self.into_workload().before_all(other)
            }

            fn after_all<Label>(self, other: impl AsLabel<Label>) -> WorkloadSystem {
                self.into_workload().after_all(other)
            }
        }
    };
}

macro_rules! system {
    ($(($type: ident, $borrow: ident, $return: ident, $index: tt))*;($type1: ident, $borrow1: ident, $return1: ident, $index1: tt) $(($queue_type: ident, $queue_borrow: ident, $queue_return: ident, $queue_index: tt))*) => {
        impl_system![$(($type, $borrow, $return, $index))*];
        system![$(($type, $borrow, $return, $index))* ($type1, $borrow1, $return1, $index1); $(($queue_type, $queue_borrow, $queue_return, $queue_index))*];
    };
    ($(($type: ident, $borrow: ident, $return: ident, $index: tt))*;) => {
        impl_system![$(($type, $borrow, $return, $index))*];
    }
}

system![(A, ViewsA, Ra, 0); (B, ViewsB, Rb, 1) (C, ViewsC, Rc, 2) (D, ViewsD, Rd, 3) (E, ViewsE, Re, 4) (F, ViewsF, Rf, 5) (G, ViewsG, Rg, 6) (H, ViewsH, Rh, 7) (I, ViewsI, Ri, 8) (J, ViewsJ, Rj, 9)
        (K, ViewsK, Rk, 10) (L, ViewsL, Rl, 11) (M, ViewsM, Rm, 12) (N, ViewsN, Rn, 13) (O, ViewsO, Ro, 14) (P, ViewsP, Rp, 15) (Q, ViewsQ, Rq, 16) (R, ViewsR, Rr, 17) (S, ViewsS, Rs, 18) (T, ViewsT, Rt, 19)];
