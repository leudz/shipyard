mod book;
mod borrow;
mod iteration;
#[cfg(feature = "serialization")]
mod serialization;
mod window;
mod workload;

use shipyard::internal::iterators;
use shipyard::prelude::*;

#[test]
fn run() {
    let world = World::new();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
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
            assert_eq!(iter.next(), Some(&mut 0));
            assert_eq!(iter.next(), Some(&mut 2));
            assert_eq!(iter.next(), None);

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
            assert_eq!(iter.next(), Some((&mut 0, &1)));
            assert_eq!(iter.next(), Some((&mut 2, &3)));
            assert_eq!(iter.next(), None);
        },
    );
}

#[cfg(feature = "parallel")]
#[test]
fn thread_pool() {
    let world = World::new();
    world.run::<(ThreadPool,), _, _>(|(thread_pool,)| {
        use rayon::prelude::*;

        let vec = vec![0, 1, 2, 3];
        thread_pool.install(|| {
            assert_eq!(vec.into_par_iter().sum::<i32>(), 6);
        });
    })
}

#[test]
fn system() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a mut usize, &'a u32);
        fn run((mut usizes, u32s): <Self::Data as SystemData>::View) {
            (&mut usizes, &u32s).iter().for_each(|(x, y)| {
                *x += *y as usize;
            });
        }
    }

    let world = World::new();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        },
    );

    world.add_workload::<System1, _>("sys1");
    world.run_default();
    world.run::<(&usize,), _, _>(|(usizes,)| {
        let mut iter = usizes.iter();
        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.next(), Some(&5));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn systems() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a mut usize, &'a u32);
        fn run((mut usizes, u32s): <Self::Data as SystemData>::View) {
            (&mut usizes, &u32s).iter().for_each(|(x, y)| {
                *x += *y as usize;
            });
        }
    }
    struct System2;
    impl<'a> System<'a> for System2 {
        type Data = (&'a mut usize,);
        fn run((mut usizes,): <Self::Data as SystemData>::View) {
            (&mut usizes,).iter().for_each(|x| {
                *x += 1;
            });
        }
    }

    let world = World::new();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        },
    );

    world.add_workload::<(System1, System2), _>("sys1");
    world.run_default();
    world.run::<(&usize,), _, _>(|(usizes,)| {
        let mut iter = usizes.iter();
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.next(), Some(&6));
        assert_eq!(iter.next(), None);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn simple_parallel_sum() {
    use rayon::prelude::*;

    let world = World::new();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (1usize, 2u32));
            entities.add_entity((&mut usizes, &mut u32s), (3usize, 4u32));
        },
    );

    world.run::<(&mut usize, ThreadPool), _, _>(|(usizes, thread_pool)| {
        thread_pool.install(|| {
            let sum: usize = (&usizes,).par_iter().cloned().sum();
            assert_eq!(sum, 4);
        });
    });
}

#[cfg(feature = "parallel")]
#[test]
fn tight_parallel_iterator() {
    use iterators::ParIter2;
    use rayon::prelude::*;

    let world = World::new();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            (&mut usizes, &mut u32s).tight_pack();
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        },
    );

    world.run::<(&mut usize, &u32, ThreadPool), _, _>(|(mut usizes, u32s, thread_pool)| {
        let counter = std::sync::atomic::AtomicUsize::new(0);
        thread_pool.install(|| {
            if let ParIter2::Tight(iter) = (&mut usizes, &u32s).par_iter() {
                iter.for_each(|(x, y)| {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    *x += *y as usize;
                });
            } else {
                panic!()
            }
        });
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
        let mut iter = (&mut usizes).iter();
        assert_eq!(iter.next(), Some(&mut 1));
        assert_eq!(iter.next(), Some(&mut 5));
        assert_eq!(iter.next(), None);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_iterator() {
    use rayon::prelude::*;

    let world = World::new();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        },
    );

    world.run::<(&mut usize, &u32, ThreadPool), _, _>(|(mut usizes, u32s, thread_pool)| {
        let counter = std::sync::atomic::AtomicUsize::new(0);
        thread_pool.install(|| {
            (&mut usizes, &u32s).par_iter().for_each(|(x, y)| {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                *x += *y as usize;
            });
        });
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
        let mut iter = (&mut usizes).iter();
        assert_eq!(iter.next(), Some(&mut 1));
        assert_eq!(iter.next(), Some(&mut 5));
        assert_eq!(iter.next(), None);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn loose_parallel_iterator() {
    use iterators::ParIter2;
    use rayon::prelude::*;

    let world = World::new();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            LoosePack::<(usize,)>::loose_pack((&mut usizes, &mut u32s));
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        },
    );

    world.run::<(&mut usize, &u32, ThreadPool), _, _>(|(mut usizes, u32s, thread_pool)| {
        let counter = std::sync::atomic::AtomicUsize::new(0);
        thread_pool.install(|| {
            if let ParIter2::Loose(iter) = (&mut usizes, &u32s).par_iter() {
                iter.for_each(|(x, y)| {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    *x += *y as usize;
                });
            } else {
                panic!()
            }
        });
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
        let mut iter = (&mut usizes).iter();
        assert_eq!(iter.next(), Some(&mut 1));
        assert_eq!(iter.next(), Some(&mut 5));
        assert_eq!(iter.next(), None);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn two_workloads() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a usize,);
        fn run(_: <Self::Data as SystemData>::View) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    let world = World::new();
    world.add_workload::<(System1,), _>("default");

    rayon::scope(|s| {
        s.spawn(|_| world.run_default());
        s.spawn(|_| world.run_default());
    });
}

#[cfg(feature = "parallel")]
#[test]
#[should_panic(
    expected = "Result::unwrap()` on an `Err` value: Cannot mutably borrow usize storage while it's already borrowed."
)]
fn two_bad_workloads() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a mut usize,);
        fn run(_: <Self::Data as SystemData>::View) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    let world = World::new();
    world.add_workload::<(System1,), _>("default");

    rayon::scope(|s| {
        s.spawn(|_| world.run_default());
        s.spawn(|_| world.run_default());
    });
}

#[test]
#[should_panic(expected = "Entity has to be alive to add component to it.")]
fn add_component_with_old_key() {
    let world = World::new();

    let entity = world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32))
        },
    );

    world.run::<AllStorages, _, _>(|mut all_storages| {
        all_storages.delete(entity);
    });

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(entities, mut usizes, mut u32s)| {
        entities.add_component((&mut usizes, &mut u32s), (1, 2), entity);
    });
}

#[test]
fn derive() {
    let t = trybuild::TestCases::new();
    t.pass("tests/derive/good.rs");
    #[cfg(feature = "parallel")]
    {
        t.pass("tests/derive/good_parallel.rs");
    }
    t.pass("tests/derive/return_nothing.rs");
    t.compile_fail("tests/derive/generic_lifetime.rs");
    t.compile_fail("tests/derive/generic_type.rs");
    #[cfg(not(any(feature = "non_send", feature = "non_sync")))]
    {
        t.compile_fail("tests/derive/not_entities.rs");
        t.compile_fail("tests/derive/unique_entities.rs");
    }
    t.compile_fail("tests/derive/not_run.rs");
    t.compile_fail("tests/derive/return_something.rs");
    t.compile_fail("tests/derive/where.rs");
    t.compile_fail("tests/derive/wrong_type.rs");
    #[cfg(feature = "non_send")]
    {
        t.pass("tests/derive/good_non_send.rs");
    }
    #[cfg(feature = "non_sync")]
    {
        t.pass("tests/derive/good_non_sync.rs");
    }
    #[cfg(all(feature = "non_send", feature = "non_sync"))]
    {
        t.pass("tests/derive/good_non_send_sync.rs");
    }
}

#[cfg(feature = "parallel")]
#[test]
fn par_update_pack() {
    use rayon::prelude::*;

    let world = World::new();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        usizes.update_pack();
        entities.add_entity(&mut usizes, 0);
        entities.add_entity(&mut usizes, 1);
        entities.add_entity(&mut usizes, 2);
        entities.add_entity(&mut usizes, 3);

        usizes.clear_inserted();

        (&usizes).par_iter().sum::<usize>();

        assert_eq!(usizes.modified().len(), 0);

        (&mut usizes).par_iter().for_each(|i| {
            *i += 1;
        });

        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), None);

        let mut iter = usizes.modified_mut().iter();
        assert_eq!(iter.next(), Some(&mut 1));
        assert_eq!(iter.next(), Some(&mut 2));
        assert_eq!(iter.next(), Some(&mut 3));
        assert_eq!(iter.next(), Some(&mut 4));
        assert_eq!(iter.next(), None);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn par_multiple_update_pack() {
    use iterators::ParIter2;
    use rayon::prelude::*;

    let world = World::new();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            u32s.update_pack();
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity(&mut usizes, 2usize);
            entities.add_entity((&mut usizes, &mut u32s), (4usize, 5u32));
            entities.add_entity(&mut u32s, 7u32);
            entities.add_entity((&mut usizes, &mut u32s), (8usize, 9u32));
            entities.add_entity((&mut usizes,), (10usize,));

            u32s.clear_inserted();
        },
    );

    world.run::<(&mut usize, &mut u32), _, _>(|(mut usizes, mut u32s)| {
        if let ParIter2::NonPacked(iter) = (&usizes, &u32s).par_iter() {
            iter.for_each(|_| {});
        } else {
            panic!("not packed");
        }

        assert_eq!(u32s.modified().len(), 0);

        if let ParIter2::NonPacked(iter) = (&mut usizes, &u32s).par_iter() {
            iter.for_each(|_| {});
        } else {
            panic!("not packed");
        }

        assert_eq!(u32s.modified().len(), 0);

        if let ParIter2::NonPacked(iter) = (&usizes, &mut u32s).par_iter() {
            iter.for_each(|_| {});
        } else {
            panic!("not packed");
        }

        let mut modified: Vec<_> = u32s.modified().iter().collect();
        modified.sort_unstable();
        assert_eq!(modified, vec![&1, &5, &7, &9]);

        let mut iter: Vec<_> = (&u32s).iter().collect();
        iter.sort_unstable();
        assert_eq!(iter, vec![&1, &5, &7, &9]);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn par_update_filter() {
    use rayon::prelude::*;

    let world = World::new();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        usizes.update_pack();
        entities.add_entity(&mut usizes, 0);
        entities.add_entity(&mut usizes, 1);
        entities.add_entity(&mut usizes, 2);
        entities.add_entity(&mut usizes, 3);

        usizes.clear_inserted();

        (&mut usizes)
            .par_iter()
            .filter(|x| **x % 2 == 0)
            .for_each(|i| {
                *i += 1;
            });

        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), None);

        let mut modified: Vec<_> = usizes.modified().iter().collect();
        modified.sort_unstable();
        assert_eq!(modified, vec![&1, &1, &3, &3]);

        let mut iter: Vec<_> = (&usizes).iter().collect();
        iter.sort_unstable();
        assert_eq!(iter, vec![&1, &1, &3, &3]);
    });
}
