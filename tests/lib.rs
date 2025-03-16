#[cfg(all(feature = "std", feature = "proc"))]
mod book;
mod borrow;
#[cfg(feature = "proc")]
mod derive;
mod iteration;
#[cfg(feature = "serde1")]
mod serde;
mod workload;

use std::iter::Sum;

use shipyard::*;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

impl Sum for USIZE {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        USIZE(iter.map(|i| i.0).sum())
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[test]
fn run() {
    let world = World::new();
    world.run(
        |(mut entities, mut usizes, mut u32s): (EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)| {
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(2), U32(3)));

            // possible to borrow twice as immutable
            let mut iter1 = (&usizes).iter();
            let _iter2 = (&usizes).iter();
            assert_eq!(iter1.next(), Some(&USIZE(0)));

            // impossible to borrow twice as mutable
            // if switched, the next two lines should trigger an shipyard::error
            let _iter = (&mut usizes).iter();
            let mut iter = (&mut usizes).iter();
            assert_eq!(iter.next().map(|x| *x), Some(USIZE(0)));
            assert_eq!(iter.next().map(|x| *x), Some(USIZE(2)));
            assert!(iter.next().is_none());

            // possible to borrow twice as immutable
            let mut iter = (&usizes, &u32s).iter();
            let _iter = (&usizes, &u32s).iter();
            assert_eq!(iter.next(), Some((&USIZE(0), &U32(1))));
            assert_eq!(iter.next(), Some((&USIZE(2), &U32(3))));
            assert_eq!(iter.next(), None);

            // impossible to borrow twice as mutable
            // if switched, the next two lines should trigger an shipyard::error
            let _iter = (&mut usizes, &u32s).iter();
            let mut iter = (&mut usizes, &u32s).iter();
            assert_eq!(iter.next().map(|(x, y)| (*x, *y)), Some((USIZE(0), U32(1))));
            assert_eq!(iter.next().map(|(x, y)| (*x, *y)), Some((USIZE(2), U32(3))));
            assert!(iter.next().is_none());
        },
    );
}

#[test]
fn system() {
    fn system1((mut usizes, u32s): (ViewMut<USIZE>, View<U32>)) {
        (&mut usizes, &u32s).iter().for_each(|(x, y)| {
            x.0 += y.0 as usize;
        });
    }

    let world = World::new();

    world.run(
        |(mut entities, mut usizes, mut u32s): (EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)| {
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(2), U32(3)));
        },
    );

    Workload::new("")
        .with_system(system1)
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
    world.run(|usizes: View<USIZE>| {
        let mut iter = usizes.iter();
        assert_eq!(iter.next(), Some(&USIZE(1)));
        assert_eq!(iter.next(), Some(&USIZE(5)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn systems() {
    fn system1((mut usizes, u32s): (ViewMut<USIZE>, View<U32>)) {
        (&mut usizes, &u32s).iter().for_each(|(x, y)| {
            x.0 += y.0 as usize;
        });
    }

    fn system2(mut usizes: ViewMut<USIZE>) {
        (&mut usizes,).iter().for_each(|x| {
            x.0 += 1;
        });
    }

    let world = World::new();

    world.run(
        |(mut entities, mut usizes, mut u32s): (EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)| {
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(2), U32(3)));
        },
    );

    Workload::new("")
        .with_system(system1)
        .with_system(system2)
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
    world.run(|usizes: View<USIZE>| {
        let mut iter = usizes.iter();
        assert_eq!(iter.next(), Some(&USIZE(2)));
        assert_eq!(iter.next(), Some(&USIZE(6)));
        assert_eq!(iter.next(), None);
    });
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn simple_parallel_sum() {
    use rayon::prelude::*;

    let world = World::new();

    world.run(
        |(mut entities, mut usizes, mut u32s): (EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)| {
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(1), U32(2)));
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(3), U32(4)));
        },
    );

    world.run(|usizes: ViewMut<USIZE>| {
        let sum: USIZE = usizes.par_iter().cloned().sum();
        assert_eq!(sum, USIZE(4));
    });
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn parallel_iterator() {
    use rayon::prelude::*;

    let world = World::new();

    world.run(
        |(mut entities, mut usizes, mut u32s): (EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)| {
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(2), U32(3)));
        },
    );

    world.run(|(mut usizes, u32s): (ViewMut<USIZE>, View<U32>)| {
        let counter = std::sync::atomic::AtomicUsize::new(0);

        (&mut usizes, &u32s).par_iter().for_each(|(x, y)| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            x.0 += y.0 as usize;
        });

        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
        let mut iter = (&mut usizes).iter();
        assert_eq!(iter.next().map(|x| *x), Some(USIZE(1)));
        assert_eq!(iter.next().map(|x| *x), Some(USIZE(5)));
        assert!(iter.next().is_none());
    });
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn two_workloads() {
    fn system1(_: View<USIZE>) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let world = World::new();
    Workload::new("")
        .with_system(system1)
        .add_to_world(&world)
        .unwrap();

    rayon::scope(|s| {
        s.spawn(|_| world.run_default_workload().unwrap());
        s.spawn(|_| world.run_default_workload().unwrap());
    });
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
#[should_panic(
    expected = "called `Result::unwrap()` on an `Err` value: System lib::two_bad_workloads::system1 failed: Cannot mutably borrow shipyard::sparse_set::SparseSet<lib::USIZE> storage while it\'s already borrowed."
)]
fn two_bad_workloads() {
    fn system1(_: ViewMut<USIZE>) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let world = World::new();
    Workload::new("")
        .with_system(system1)
        .add_to_world(&world)
        .unwrap();

    rayon::scope(|s| {
        s.spawn(|_| world.run_default_workload().unwrap());
        s.spawn(|_| world.run_default_workload().unwrap());
    });
}

#[test]
#[should_panic(expected = "Entity has to be alive to add component to it.")]
fn add_component_with_old_key() {
    let world = World::new();

    let entity = {
        let (mut entities, mut usizes, mut u32s) = world
            .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>()
            .unwrap();
        entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)))
    };

    world.run(|mut all_storages: AllStoragesViewMut| {
        all_storages.delete_entity(entity);
    });

    let (entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>()
        .unwrap();

    entities.add_component(entity, (&mut usizes, &mut u32s), (USIZE(1), U32(2)));
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn par_update_pack() {
    #[derive(PartialEq, Eq, Debug, Clone, Copy)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    impl Sum for USIZE {
        fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
            USIZE(iter.map(|i| i.0).sum())
        }
    }
    impl<'a> Sum<&'a USIZE> for USIZE {
        fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
            USIZE(iter.map(|i| i.0).sum())
        }
    }

    use rayon::prelude::*;

    let mut world = World::new();
    world.track_all::<USIZE>();

    world.run(
        |(mut entities, mut usizes): (EntitiesViewMut, ViewMut<USIZE, track::All>)| {
            entities.add_entity(&mut usizes, USIZE(0));
            entities.add_entity(&mut usizes, USIZE(1));
            entities.add_entity(&mut usizes, USIZE(2));
            entities.add_entity(&mut usizes, USIZE(3));

            usizes.clear_all_inserted();
        },
    );

    world.run(|mut usizes: ViewMut<USIZE, track::All>| {
        (&usizes).par_iter().sum::<USIZE>();

        assert_eq!(usizes.modified().iter().count(), 0);

        (&mut usizes).par_iter().for_each(|mut i| {
            i.0 += 1;
        });

        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), None);

        let mut iter = usizes.modified_mut().iter();
        assert_eq!(iter.next().map(|x| *x), Some(USIZE(1)));
        assert_eq!(iter.next().map(|x| *x), Some(USIZE(2)));
        assert_eq!(iter.next().map(|x| *x), Some(USIZE(3)));
        assert_eq!(iter.next().map(|x| *x), Some(USIZE(4)));
        assert!(iter.next().is_none());
    });
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn par_multiple_update_pack() {
    #[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
    struct U32(u32);
    impl Component for U32 {
        type Tracking = track::Untracked;
    }

    use rayon::prelude::*;

    let mut world = World::new();
    world.track_all::<U32>();

    world.run(
        |(mut entities, mut usizes, mut u32s): (
            EntitiesViewMut,
            ViewMut<USIZE>,
            ViewMut<U32, track::All>,
        )| {
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
            entities.add_entity(&mut usizes, USIZE(2));
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(4), U32(5)));
            entities.add_entity(&mut u32s, U32(7));
            entities.add_entity((&mut usizes, &mut u32s), (USIZE(8), U32(9)));
            entities.add_entity((&mut usizes,), (USIZE(10),));

            u32s.clear_all_inserted();
        },
    );

    world.run(
        |(mut usizes, mut u32s): (ViewMut<USIZE>, ViewMut<U32, track::All>)| {
            (&usizes, &u32s).par_iter().for_each(|_| {});

            assert_eq!(u32s.modified().iter().count(), 0);

            (&mut usizes, &u32s).par_iter().for_each(|(x, y)| {
                x.0 += y.0 as usize;
                x.0 -= y.0 as usize;
            });

            assert_eq!(u32s.modified().iter().count(), 0);

            (&usizes, &mut u32s).par_iter().for_each(|(x, mut y)| {
                y.0 += x.0 as u32;
                y.0 -= x.0 as u32;
            });

            let mut modified: Vec<_> = u32s.modified().iter().collect();
            modified.sort_unstable();
            assert_eq!(modified, vec![&U32(1), &U32(5), &U32(9)]);

            let mut iter: Vec<_> = (&u32s).iter().collect();
            iter.sort_unstable();
            assert_eq!(iter, vec![&U32(1), &U32(5), &U32(7), &U32(9)]);
        },
    );
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn par_update_filter() {
    #[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    use rayon::prelude::*;

    let mut world = World::new();
    world.track_all::<USIZE>();

    world.run(
        |(mut entities, mut usizes): (EntitiesViewMut, ViewMut<USIZE, track::All>)| {
            entities.add_entity(&mut usizes, USIZE(0));
            entities.add_entity(&mut usizes, USIZE(1));
            entities.add_entity(&mut usizes, USIZE(2));
            entities.add_entity(&mut usizes, USIZE(3));

            usizes.clear_all_inserted();
        },
    );

    world.run(|mut usizes: ViewMut<USIZE, track::All>| {
        (&mut usizes)
            .par_iter()
            .filter(|x| x.0 % 2 == 0)
            .for_each(|mut i| {
                i.0 += 1;
            });

        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), None);

        let mut modified: Vec<_> = usizes.modified().iter().collect();
        modified.sort_unstable();
        assert_eq!(modified, vec![&USIZE(1), &USIZE(3)]);

        let mut iter: Vec<_> = (&usizes).iter().collect();
        iter.sort_unstable();
        assert_eq!(iter, vec![&USIZE(1), &USIZE(1), &USIZE(3), &USIZE(3)]);
    });
}

#[test]
fn contains() {
    let world = World::new();

    world.run(
        |mut entities: EntitiesViewMut, mut usizes: ViewMut<USIZE>, mut u32s: ViewMut<U32>| {
            let entity = entities.add_entity((), ());

            entities.add_component(entity, &mut usizes, USIZE(0));

            assert!(usizes.contains(entity));
            assert!(!(&usizes, &u32s).contains(entity));

            entities.add_component(entity, &mut u32s, U32(1));

            assert!((&usizes, &u32s).contains(entity));
        },
    );
}

#[test]
fn debug() {
    let mut world = World::new();

    world.add_entity((USIZE(0),));
    world.add_entity((USIZE(1),));
    world.add_entity((USIZE(2),));

    world.run(|usizes: View<USIZE>| {
        assert_eq!(
            format!("{:?}", usizes),
            "[(EId(0.0), USIZE(0)), (EId(1.0), USIZE(1)), (EId(2.0), USIZE(2))]"
        );
    });
}
