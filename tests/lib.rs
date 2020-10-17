#[cfg(feature = "panic")]
mod book;
mod borrow;
mod iteration;
#[cfg(feature = "serde1")]
mod serde;
mod workload;

use shipyard::error;
use shipyard::*;

#[test]
fn run() {
    let world = World::new();
    world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));

                // possible to borrow twice as immutable
                let mut iter1 = (&usizes).iter();
                let _iter2 = (&usizes).iter();
                assert_eq!(iter1.next(), Some(&0));

                // impossible to borrow twice as mutable
                // if switched, the next two lines should trigger an shipyard::error
                let _iter = (&mut usizes).iter();
                let mut iter = (&mut usizes).iter();
                assert_eq!(iter.next().map(|x| *x), Some(0));
                assert_eq!(iter.next().map(|x| *x), Some(2));
                assert!(iter.next().is_none());

                // possible to borrow twice as immutable
                let mut iter = (&usizes, &u32s).iter();
                let _iter = (&usizes, &u32s).iter();
                assert_eq!(iter.next(), Some((&0, &1)));
                assert_eq!(iter.next(), Some((&2, &3)));
                assert_eq!(iter.next(), None);

                // impossible to borrow twice as mutable
                // if switched, the next two lines should trigger an shipyard::error
                let _iter = (&mut usizes, &u32s).iter();
                let mut iter = (&mut usizes, &u32s).iter();
                assert_eq!(iter.next().map(|(x, y)| (*x, *y)), Some((0, 1)));
                assert_eq!(iter.next().map(|(x, y)| (*x, *y)), Some((2, 3)));
                assert!(iter.next().is_none());
            },
        )
        .unwrap();
}

#[test]
fn system() {
    fn system1((mut usizes, u32s): (ViewMut<usize>, View<u32>)) {
        (&mut usizes, &u32s).iter().for_each(|(mut x, y)| {
            *x += *y as usize;
        });
    }

    let world = World::new();

    world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
            },
        )
        .unwrap();

    Workload::builder("")
        .try_with_system(system!(system1))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    world.try_run_default().unwrap();
    world
        .try_run(|usizes: View<usize>| {
            let mut iter = usizes.iter();
            assert_eq!(iter.next(), Some(&1));
            assert_eq!(iter.next(), Some(&5));
            assert_eq!(iter.next(), None);
        })
        .unwrap();
}

#[test]
fn systems() {
    fn system1((mut usizes, u32s): (ViewMut<usize>, View<u32>)) {
        (&mut usizes, &u32s).iter().for_each(|(mut x, y)| {
            *x += *y as usize;
        });
    }

    fn system2(mut usizes: ViewMut<usize>) {
        (&mut usizes,).iter().for_each(|(mut x,)| {
            *x += 1;
        });
    }

    let world = World::new();

    world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
            },
        )
        .unwrap();

    Workload::builder("")
        .try_with_system(system!(system1))
        .unwrap()
        .try_with_system(system!(system2))
        .unwrap()
        .add_to_world(&world)
        .unwrap();

    world.try_run_default().unwrap();
    world
        .try_run(|usizes: View<usize>| {
            let mut iter = usizes.iter();
            assert_eq!(iter.next(), Some(&2));
            assert_eq!(iter.next(), Some(&6));
            assert_eq!(iter.next(), None);
        })
        .unwrap();
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn simple_parallel_sum() {
    use rayon::prelude::*;

    let world = World::new();

    world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                entities.add_entity((&mut usizes, &mut u32s), (1usize, 2u32));
                entities.add_entity((&mut usizes, &mut u32s), (3usize, 4u32));
            },
        )
        .unwrap();

    world
        .try_run(|usizes: ViewMut<usize>| {
            let sum: usize = usizes.par_iter().cloned().sum();
            assert_eq!(sum, 4);
        })
        .unwrap();
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn parallel_iterator() {
    use rayon::prelude::*;

    let world = World::new();

    world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
            },
        )
        .unwrap();

    world
        .try_run(|(mut usizes, u32s): (ViewMut<usize>, View<u32>)| {
            let counter = std::sync::atomic::AtomicUsize::new(0);

            (&mut usizes, &u32s).par_iter().for_each(|(mut x, y)| {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                *x += *y as usize;
            });

            assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
            let mut iter = (&mut usizes).iter();
            assert_eq!(iter.next().map(|x| *x), Some(1));
            assert_eq!(iter.next().map(|x| *x), Some(5));
            assert!(iter.next().is_none());
        })
        .unwrap();
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn two_workloads() {
    fn system1(_: View<usize>) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let world = World::new();
    Workload::builder("")
        .with_system(system!(system1))
        .add_to_world(&world)
        .unwrap();

    rayon::scope(|s| {
        s.spawn(|_| world.try_run_default().unwrap());
        s.spawn(|_| world.try_run_default().unwrap());
    });
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
#[should_panic(
    expected = "called `Result::unwrap()` on an `Err` value: System lib::two_bad_workloads::system1 failed: Cannot mutably borrow shipyard::sparse_set::SparseSet<usize> storage while it\'s already borrowed."
)]
fn two_bad_workloads() {
    fn system1(_: ViewMut<usize>) {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let world = World::new();
    Workload::builder("")
        .with_system(system!(system1))
        .add_to_world(&world)
        .unwrap();

    rayon::scope(|s| {
        s.spawn(|_| world.try_run_default().unwrap());
        s.spawn(|_| world.try_run_default().unwrap());
    });
}

#[test]
fn add_component_with_old_key() {
    let world = World::new();

    let entity = {
        let (mut entities, mut usizes, mut u32s) = world
            .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
            .unwrap();
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32))
    };

    world
        .try_run(|mut all_storages: AllStoragesViewMut| {
            all_storages.delete(entity);
        })
        .unwrap();

    let (entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();
    assert_eq!(
        entities.try_add_component(entity, (&mut usizes, &mut u32s), (1, 2)),
        Err(error::AddComponent::EntityIsNotAlive)
    );
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn par_update_pack() {
    use rayon::prelude::*;

    let world = World::new();

    world
        .try_run(
            |(mut entities, mut usizes): (EntitiesViewMut, ViewMut<usize>)| {
                usizes.update_pack();
                entities.add_entity(&mut usizes, 0);
                entities.add_entity(&mut usizes, 1);
                entities.add_entity(&mut usizes, 2);
                entities.add_entity(&mut usizes, 3);

                usizes.try_clear_inserted().unwrap();

                (&usizes).par_iter().sum::<usize>();

                assert_eq!(usizes.modified().iter().count(), 0);

                (&mut usizes).par_iter().for_each(|mut i| {
                    *i += 1;
                });

                let mut iter = usizes.inserted().iter();
                assert_eq!(iter.next(), None);

                let mut iter = usizes.modified_mut().iter();
                assert_eq!(iter.next().map(|x| *x), Some(1));
                assert_eq!(iter.next().map(|x| *x), Some(2));
                assert_eq!(iter.next().map(|x| *x), Some(3));
                assert_eq!(iter.next().map(|x| *x), Some(4));
                assert!(iter.next().is_none());
            },
        )
        .unwrap();
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn par_multiple_update_pack() {
    use rayon::prelude::*;

    let world = World::new();

    world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                u32s.update_pack();
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
                entities.add_entity(&mut usizes, 2usize);
                entities.add_entity((&mut usizes, &mut u32s), (4usize, 5u32));
                entities.add_entity(&mut u32s, 7u32);
                entities.add_entity((&mut usizes, &mut u32s), (8usize, 9u32));
                entities.add_entity((&mut usizes,), (10usize,));

                u32s.try_clear_inserted().unwrap();
            },
        )
        .unwrap();

    world
        .try_run(|(mut usizes, mut u32s): (ViewMut<usize>, ViewMut<u32>)| {
            if let iter::ParIter::Mixed(iter) = (&usizes, &u32s).par_iter() {
                iter.for_each(|_| {});
            } else {
                panic!("not packed");
            }

            assert_eq!(u32s.modified().iter().count(), 0);

            if let iter::ParIter::Mixed(iter) = (&mut usizes, &u32s).par_iter() {
                iter.for_each(|(mut x, y)| {
                    *x += *y as usize;
                    *x -= *y as usize;
                });
            } else {
                panic!("not packed");
            }

            assert_eq!(u32s.modified().iter().count(), 0);

            if let iter::ParIter::Mixed(iter) = (&usizes, &mut u32s).par_iter() {
                iter.for_each(|(x, mut y)| {
                    *y += *x as u32;
                    *y -= *x as u32;
                });
            } else {
                panic!("not packed");
            }

            let mut modified: Vec<_> = u32s.modified().iter().collect();
            modified.sort_unstable();
            assert_eq!(modified, vec![&1, &5, &9]);

            let mut iter: Vec<_> = (&u32s).iter().collect();
            iter.sort_unstable();
            assert_eq!(iter, vec![&1, &5, &7, &9]);
        })
        .unwrap();
}

#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
#[test]
fn par_update_filter() {
    use rayon::prelude::*;

    let world = World::new();

    world
        .try_run(
            |(mut entities, mut usizes): (EntitiesViewMut, ViewMut<usize>)| {
                usizes.update_pack();
                entities.add_entity(&mut usizes, 0);
                entities.add_entity(&mut usizes, 1);
                entities.add_entity(&mut usizes, 2);
                entities.add_entity(&mut usizes, 3);

                usizes.try_clear_inserted().unwrap();

                (&mut usizes)
                    .par_iter()
                    .filter(|x| **x % 2 == 0)
                    .for_each(|mut i| {
                        *i += 1;
                    });

                let mut iter = usizes.inserted().iter();
                assert_eq!(iter.next(), None);

                let mut modified: Vec<_> = usizes.modified().iter().collect();
                modified.sort_unstable();
                assert_eq!(modified, vec![&1, &3]);

                let mut iter: Vec<_> = (&usizes).iter().collect();
                iter.sort_unstable();
                assert_eq!(iter, vec![&1, &1, &3, &3]);
            },
        )
        .unwrap();
}

#[test]
fn contains() {
    let world = World::new();

    world
        .try_run(
            |mut entities: EntitiesViewMut, mut usizes: ViewMut<usize>, mut u32s: ViewMut<u32>| {
                let entity = entities.add_entity((), ());

                entities.try_add_component(entity, &mut usizes, 0).unwrap();

                assert!(usizes.contains(entity));
                assert!(!(&usizes, &u32s).contains(entity));

                entities.try_add_component(entity, &mut u32s, 1).unwrap();

                assert!((&usizes, &u32s).contains(entity));
            },
        )
        .unwrap();
}
