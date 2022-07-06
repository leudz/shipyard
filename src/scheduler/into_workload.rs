use crate::info::DedupedLabels;
use crate::scheduler::label::SequentialLabel;
use crate::scheduler::workload::Workload;
use crate::scheduler::IntoWorkloadSystem;
use crate::type_id::TypeId;
use crate::AsLabel;
use alloc::vec::Vec;
// macro not module
use alloc::boxed::Box;
use alloc::vec;

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
    /// Converts to a collection of systems.  
    /// All systems will run one after the other. Does not propagate into nested [`Workload`] but they will run sequentially between them.
    ///
    /// Not different than [`into_workload`](IntoWorkload::into_workload) for a single system.
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
    /// (workload1(), sys3, sys4).into_sequential_workload();
    /// ```
    ///
    /// In this example `sys1` and `sys2` can run in parallel but always before `sys3`.  
    /// `sys3` and `sys4` run sequentially.
    fn into_sequential_workload(self) -> Workload;
}

impl IntoWorkload<(), ()> for Workload {
    fn into_workload(self) -> Workload {
        self
    }
    fn into_sequential_workload(mut self) -> Workload {
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
{
    fn into_workload(self) -> Workload {
        Workload {
            name: Box::new(TypeId::of::<Sys>()),
            systems: vec![self.into_workload_system().unwrap()],
            run_if: None,
            tags: vec![Box::new(TypeId::of::<Sys>())],
            before_all: DedupedLabels::new(),
            after_all: DedupedLabels::new(),
            overwritten_name: false,
            require_before: DedupedLabels::new(),
            require_after: DedupedLabels::new(),
        }
    }

    fn into_sequential_workload(self) -> Workload {
        self.into_workload()
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
                let mut workload = Workload {
                    name: Box::new(TypeId::of::<($($type,)+)>()),
                    systems: Vec::new(),
                    run_if: None,
                    before_all: DedupedLabels::new(),
                    after_all: DedupedLabels::new(),
                    tags: vec![Box::new(TypeId::of::<($($type,)+)>())],
                    overwritten_name: false,
                    require_before: DedupedLabels::new(),
                    require_after: DedupedLabels::new(),
                };

                $(
                    let mut w = self.$index.into_workload();
                    workload = workload.merge(&mut w);
                )+

                workload
            }

            fn into_sequential_workload(self) -> Workload {
                let mut workload = Workload {
                    name: Box::new(TypeId::of::<($($type,)+)>()),
                    systems: Vec::new(),
                    run_if: None,
                    before_all: DedupedLabels::new(),
                    after_all: DedupedLabels::new(),
                    tags: vec![Box::new(TypeId::of::<($($type,)+)>())],
                    overwritten_name: false,
                    require_before: DedupedLabels::new(),
                    require_after: DedupedLabels::new(),
                };

                let mut sequential_tags = Vec::new();

                let mut workloads = ($({
                    let mut w = self.$index.into_workload();

                    let tag = SequentialLabel(w.name.clone());
                    w = w.tag(tag.clone());

                    sequential_tags.push(SequentialLabel(w.name.clone()));

                    w
                },)+);

                $(
                    if let Some(sequential_tag) = sequential_tags.get($index + 1) {
                        workloads.$index = workloads.$index.before_all(sequential_tag.clone());
                    }

                    workload = workload.merge(&mut workloads.$index);
                )+

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

into_workload![(A, ViewsA, Ra, 0); (B, ViewsB, Rb, 1) (C, ViewsC, Rc, 2) (D, ViewsD, Rd, 3) (E, ViewsE, Re, 4) (F, ViewsF, Rf, 5) (G, ViewsG, Rg, 6) (H, ViewsH, Rh, 7) (I, ViewsI, Ri, 8) (J, ViewsJ, Rj, 9)
        (K, ViewsK, Rk, 10) (L, ViewsL, Rl, 11) (M, ViewsM, Rm, 12) (N, ViewsN, Rn, 13) (O, ViewsO, Ro, 14) (P, ViewsP, Rp, 15) (Q, ViewsQ, Rq, 16) (R, ViewsR, Rr, 17) (S, ViewsS, Rs, 18) (T, ViewsT, Rt, 19)];
