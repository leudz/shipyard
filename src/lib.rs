//! # Getting started
//! ```
//! use shipyard::*;
//!
//! struct Health(f32);
//! struct Position { x: f32, y: f32 };
//!
//! struct InAcid;
//! impl<'a> System<'a> for InAcid {
//!     type Data = (&'a Position, &'a mut Health);
//!     fn run(&self, (pos, mut health): <Self::Data as SystemData>::View) {
//!         for (pos, health) in (&pos, &mut health).iter() {
//!             if is_in_acid(pos) {
//!                 health.0 -= 1.0;
//!             }
//!         }
//!     }
//! }
//!
//! fn is_in_acid(pos: &Position) -> bool {
//!     // well... it's wet season
//!      
//!     true
//! }
//!
//! let world = World::default();
//!
//! world.new_entity((Position { x: 0.0, y: 0.0 }, Health(1000.0)));
//!
//! world.add_workload("In acid", InAcid);
//! world.run_default();
//! ```
//! # Let's make some pigs!
//! ```
//! # #[cfg(feature = "parallel")]
//! # {
//! use shipyard::*;
//! use iterators::Iter;
//!
//! struct Health(f32);
//! struct Fat(f32);
//!
//! struct Reproduction;
//! impl<'a> System<'a> for Reproduction {
//!     type Data = (&'a mut Fat, &'a mut Health, EntitiesMut);
//!     fn run(&self, (mut fat, mut health, mut entities): <Self::Data as SystemData>::View) {
//!         let count = (&health, &fat).iter().filter(|(health, fat)| health.0 > 40.0 && fat.0 > 20.0).count();
//!         (0..count).for_each(|_| {
//!             entities.add_entity((&mut health, &mut fat), (Health(100.0), Fat(0.0)));
//!         });
//!     }
//! }
//!
//! struct Meal;
//! impl<'a> System<'a> for Meal {
//!     type Data = &'a mut Fat;
//!     fn run(&self, mut fat: <Self::Data as SystemData>::View) {
//!         if let Iter::Packed(iter) = fat.iter() {
//!             for slice in iter.into_chunk(8) {
//!                 for fat in slice {
//!                     fat.0 += 3.0;
//!                 }
//!             }
//!         }
//!     }
//! }
//!
//! struct Age;
//! impl<'a> System<'a> for Age {
//!     type Data = (&'a mut Health, ThreadPool);
//!     fn run(&self, (mut health, thread_pool): <Self::Data as SystemData>::View) {
//!         use rayon::prelude::ParallelIterator;
//!
//!         thread_pool.install(|| {
//!             health.par_iter().for_each(|health| {
//!                 health.0 -= 4.0;
//!             });
//!         });
//!     }
//! }
//!
//! let world = World::new::<(Health, Fat)>();
//!
//! world.run::<(EntitiesMut, &mut Health, &mut Fat), _>(|(mut entities, mut health, mut fat)| {
//!     (0..100).for_each(|_| {
//!         entities.add_entity((&mut health, &mut fat), (Health(100.0), Fat(0.0)));
//!     })
//! });
//!
//! world.add_workload("Life", (Meal, Age));
//! world.add_workload("Reproduction", Reproduction);
//!
//! for day in 0..100 {
//!     if day % 6 == 0 {
//!         world.run_workload("Reproduction");
//!     }
//!     world.run_default();
//! }
//!
//! // we've got some new pigs
//! assert_eq!(world.get_storage::<&Health>().len(), 900);
//! # }
//! ```

#![deny(bare_trait_objects)]

mod add_entity;
mod atomic_refcell;
mod component_storage;
mod entity;
pub mod error;
mod get;
mod get_storage;
pub mod iterators;
mod not;
mod pack;
mod remove;
mod run;
mod sparse_array;
mod world;

pub use crate::component_storage::AllStorages;
pub use crate::get::GetComponent;
pub use crate::not::Not;
pub use crate::pack::OwnedPack;
pub use crate::remove::Remove;
pub use crate::run::System;
#[doc(hidden)]
pub use crate::run::SystemData;
pub use crate::world::World;
pub use entity::{Entities, EntitiesMut, EntitiesViewMut};
pub use iterators::IntoIter;

/// Type used to borrow the rayon::ThreadPool inside `World`.
#[cfg(feature = "parallel")]
pub struct ThreadPool;

#[cfg(test)]
mod test {
    use super::iterators::*;
    use super::*;
    #[test]
    fn new_entity() {
        let world = World::new::<(usize, u32)>();
        let entity1 = world.new_entity((0usize, 1u32));
        let usizes = world.get_storage::<&usize>();
        let u32s = world.get_storage::<&u32>();
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
    }
    #[test]
    fn indirect_new_entity() {
        let world = World::default();
        let entity1 = world.new_entity((0usize, 1u32));
        let storages = world.all_storages();
        let (usizes, u32s) = storages.get_storage::<(&usize, &u32)>();
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
    }
    #[test]
    fn add_entity() {
        let world = World::default();
        world.register::<usize>();
        world.register::<u32>();
        let mut entities = world.entities_mut();
        let (mut usizes, mut u32s) = world.get_storage::<(&mut usize, &mut u32)>();
        let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
    }
    #[test]
    fn add_component() {
        let world = World::default();
        world.register::<usize>();
        world.register::<u32>();
        let mut entities = world.entities_mut();
        let (mut usizes, mut u32s) = world.get_storage::<(&mut usize, &mut u32)>();
        let entity1 = entities.add_entity((), ());
        entities.add_component((&mut *usizes, &mut *u32s), (0, 1), entity1);
        entities.add_component((&mut usizes, &mut u32s), (2, 3), entity1);
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
    }
    #[test]
    fn run() {
        let world = World::new::<(usize, u32)>();
        world.run::<(EntitiesMut, &mut usize, &mut u32), _>(
            |(mut entities, mut usizes, mut u32s)| {
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));

                // possible to borrow twice as immutable
                let mut iter1 = (&usizes).iter();
                let _iter2 = (&usizes).iter();
                assert_eq!(iter1.next(), Some(&0));

                // impossible to borrow twice as mutable
                // if switched, the next two lines should trigger an error
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
                // if switched, the next two lines should trigger an error
                let _iter = (&mut usizes, &u32s).iter();
                let mut iter = (&mut usizes, &u32s).iter();
                assert_eq!(iter.next(), Some((&mut 0, &1)));
                assert_eq!(iter.next(), Some((&mut 2, &3)));
                assert_eq!(iter.next(), None);
            },
        );
    }
    #[test]
    fn iterators() {
        let world = World::new::<(usize, u32)>();
        world.run::<(EntitiesMut, &mut usize, &mut u32), _>(
            |(mut entities, mut usizes, mut u32s)| {
                let entity1 = entities.add_entity((&mut usizes,), (0usize,));
                entities.add_component((&mut u32s,), (1u32,), entity1);
                entities.add_entity((&mut usizes,), (2usize,));

                let mut iter = (&usizes).iter();
                assert_eq!(iter.next(), Some(&0));
                assert_eq!(iter.next(), Some(&2));
                assert_eq!(iter.next(), None);

                let mut iter = (&usizes, &u32s).iter();
                assert_eq!(iter.next(), Some((&0, &1)));
                assert_eq!(iter.next(), None);
            },
        );
    }
    #[test]
    fn not_iterators() {
        let world = World::new::<(usize, u32)>();
        world.new_entity((0usize, 1u32));
        world.new_entity((2usize,));
        world.run::<(Not<&usize>, &u32), _>(|(not_usizes, u32s)| {
            let mut iter = (&not_usizes).iter();
            assert_eq!(iter.next(), None);

            let mut iter = (&not_usizes, !&u32s).iter();
            assert_eq!(iter.next(), None);

            let mut iter = (&not_usizes, &u32s).iter();
            assert_eq!(iter.next(), None);

            let usizes = not_usizes.into_inner();

            let mut iter = (&usizes, !&u32s).iter();
            assert_eq!(iter.next(), Some((&2, ())));
            assert_eq!(iter.next(), None);
        });
    }
    #[test]
    fn not() {
        let world = World::new::<(usize, u32)>();
        let entity1 = world.new_entity((0usize, 1u32));
        let entity2 = world.new_entity((2usize,));
        let entity3 = world.new_entity((3u32,));
        let (usizes, not_u32s) = world.get_storage::<(&usize, &u32)>();
        let storages = (&usizes, &!not_u32s);
        assert_eq!(storages.get(entity1), None);
        assert_eq!(storages.get(entity2), Some((&2usize, ())));
        assert_eq!(storages.get(entity3), None);
    }
    #[test]
    fn pack() {
        let world = World::new::<(usize, u32)>();
        let entity = world.new_entity(());
        let (mut usizes, mut u32s) = world.get_storage::<(&mut usize, &mut u32)>();
        (&mut usizes, &mut u32s).pack_owned();

        let entities = world.entities();
        entities.add_component((&mut usizes, &mut u32s), (0, 1), entity);
        entities.add_component((&mut usizes, &mut u32s), (2,), entity);
    }
    #[test]
    fn pack_missing_storage() {
        match std::panic::catch_unwind(|| {
            let world = World::new::<(usize, u32)>();
            let entity = world.new_entity(());
            world.pack_owned::<(usize, u32)>();
            let (mut usizes,) = world.get_storage::<(&mut usize,)>();

            let entities = world.entities();
            entities.add_component(&mut *usizes, 0, entity);
        }) {
            Ok(_) => panic!(),
            Err(err) => assert_eq!(format!("{}", err.downcast::<String>().unwrap()), format!("called `Result::unwrap()` on an `Err` value: Missing storage of type ({:?}). To add a packed component you have to pass all storages packed with it. Even if you just add one component.", std::any::TypeId::of::<usize>())),
        }
    }
    #[test]
    fn pack_iterator() {
        let world = World::new::<(usize, u32)>();
        world.pack_owned::<(usize, u32)>();
        world.new_entity((0usize, 1u32));
        world.new_entity((2usize,));
        world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
            if let Iter::Packed(mut iter) = (usizes, u32s).iter() {
                assert_eq!(iter.next(), Some((&0, &1)));
                assert_eq!(iter.next(), None);
            } else {
                panic!("not packed");
            }
        });
    }
    #[test]
    fn chunk_iterator() {
        let world = World::new::<(usize, u32)>();
        world.pack_owned::<(usize, u32)>();
        world.new_entity((0usize, 1u32));
        world.new_entity((2usize, 3u32));
        world.new_entity((4usize, 5u32));
        world.new_entity((6usize, 7u32));
        world.new_entity((8usize, 9u32));
        world.new_entity((10usize,));
        world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
            if let Iter::Packed(iter) = (&usizes, &u32s).iter() {
                let mut iter = iter.into_chunk(2);
                assert_eq!(iter.next(), Some((&[0, 2][..], &[1, 3][..])));
                assert_eq!(iter.next(), Some((&[4, 6][..], &[5, 7][..])));
                assert_eq!(iter.next(), Some((&[8][..], &[9][..])));
                assert_eq!(iter.next(), None);
            } else {
                panic!("not packed");
            }
            if let Iter::Packed(iter) = (&usizes, &u32s).iter() {
                let mut iter = iter.into_chunk_exact(2);
                assert_eq!(iter.next(), Some((&[0, 2][..], &[1, 3][..])));
                assert_eq!(iter.next(), Some((&[4, 6][..], &[5, 7][..])));
                assert_eq!(iter.next(), None);
                assert_eq!(iter.remainder(), (&[8][..], &[9][..]));
                assert_eq!(iter.remainder(), (&[][..], &[][..]));
                assert_eq!(iter.next(), None);
            } else {
                panic!("not packed");
            }
            if let Iter::Packed(mut iter) = (&usizes, &u32s).iter() {
                iter.next();
                let mut iter = iter.into_chunk(2);
                assert_eq!(iter.next(), Some((&[2, 4][..], &[3, 5][..])));
                assert_eq!(iter.next(), Some((&[6, 8][..], &[7, 9][..])));
                assert_eq!(iter.next(), None);
            } else {
                panic!("not packed");
            }
            if let Iter::Packed(mut iter) = (&usizes, &u32s).iter() {
                iter.next();
                let mut iter = iter.into_chunk_exact(2);
                assert_eq!(iter.next(), Some((&[2, 4][..], &[3, 5][..])));
                assert_eq!(iter.next(), Some((&[6, 8][..], &[7, 9][..])));
                assert_eq!(iter.remainder(), (&[][..], &[][..]));
                assert_eq!(iter.next(), None);
            } else {
                panic!("not packed");
            }
            if let Iter::Packed(mut iter) = (&usizes, &u32s).iter() {
                iter.next();
                iter.next();
                let mut iter = iter.into_chunk_exact(2);
                assert_eq!(iter.next(), Some((&[4, 6][..], &[5, 7][..])));
                assert_eq!(iter.next(), None);
                assert_eq!(iter.remainder(), (&[8][..], &[9][..]));
                assert_eq!(iter.remainder(), (&[][..], &[][..]));
                assert_eq!(iter.next(), None);
            } else {
                panic!("not packed");
            }
            if let Iter::Packed(mut iter) = (&usizes, &u32s).iter() {
                iter.nth(3);
                let mut iter = iter.into_chunk_exact(2);
                assert_eq!(iter.next(), None);
                assert_eq!(iter.remainder(), (&[8][..], &[9][..]));
                assert_eq!(iter.remainder(), (&[][..], &[][..]));
                assert_eq!(iter.next(), None);
            } else {
                panic!("not packed");
            }
        });
    }
    #[test]
    fn remove() {
        let world = World::new::<(usize, u32)>();
        let entity1 = world.new_entity((0usize, 1u32));
        let entity2 = world.new_entity((2usize, 3u32));
        let (mut usizes, mut u32s) = world.get_storage::<(&mut usize, &mut u32)>();
        let component = Remove::<(usize,)>::remove((&mut usizes,), entity1);
        assert_eq!(component, (Some(0usize),));
        assert_eq!((&mut usizes).get(entity1), None);
        assert_eq!((&mut u32s).get(entity1), Some(&mut 1));
        assert_eq!(usizes.get(entity2), Some(&2));
        assert_eq!(u32s.get(entity2), Some(&3));
    }
    #[test]
    fn remove_packed() {
        let world = World::new::<(usize, u32)>();
        world.pack_owned::<(usize, u32)>();
        let entity1 = world.new_entity((0usize, 1u32));
        let entity2 = world.new_entity((2usize, 3u32));
        let (mut usizes, mut u32s) = world.get_storage::<(&mut usize, &mut u32)>();
        let component = Remove::<(usize,)>::remove((&mut usizes, &mut u32s), entity1);
        assert_eq!(component, (Some(0usize),));
        assert_eq!((&mut usizes).get(entity1), None);
        assert_eq!((&mut u32s).get(entity1), Some(&mut 1));
        assert_eq!(usizes.get(entity2), Some(&2));
        assert_eq!(u32s.get(entity2), Some(&3));
    }
    #[test]
    fn delete() {
        let world = World::new::<(usize, u32)>();
        let entity1 = world.new_entity((0usize, 1u32));
        let entity2 = world.new_entity((2usize, 3u32));
        assert!(world.delete(entity1));
        assert!(!world.delete(entity1));
        let (usizes, u32s) = world.get_storage::<(&usize, &u32)>();
        assert_eq!((&usizes).get(entity1), None);
        assert_eq!((&u32s).get(entity1), None);
        assert_eq!(usizes.get(entity2), Some(&2));
        assert_eq!(u32s.get(entity2), Some(&3));
    }
    #[cfg(feature = "parallel")]
    #[test]
    fn thread_pool() {
        let world = World::new::<(usize, u32)>();
        world.run::<(ThreadPool,), _>(|(thread_pool,)| {
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
            fn run(&self, (usizes, u32s): <Self::Data as SystemData>::View) {
                for (x, y) in (usizes, u32s).iter() {
                    *x += *y as usize;
                }
            }
        }
        let world = World::new::<(usize, u32)>();
        world.new_entity((0usize, 1u32));
        world.new_entity((2usize, 3u32));
        world.add_workload("sys1", System1);
        world.run_default();
        world.run::<(&usize,), _>(|(usizes,)| {
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
            fn run(&self, (usizes, u32s): <Self::Data as SystemData>::View) {
                for (x, y) in (usizes, u32s).iter() {
                    *x += *y as usize;
                }
            }
        }
        struct System2;
        impl<'a> System<'a> for System2 {
            type Data = (&'a mut usize,);
            fn run(&self, (usizes,): <Self::Data as SystemData>::View) {
                for x in (usizes,).iter() {
                    *x += 1;
                }
            }
        }
        let world = World::new::<(usize, u32)>();
        world.new_entity((0usize, 1u32));
        world.new_entity((2usize, 3u32));
        world.add_workload("sys1", (System1, System2));
        world.run_default();
        world.run::<(&usize,), _>(|(usizes,)| {
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

        let world = World::new::<(usize, u32)>();
        world.new_entity((1usize, 2u32));
        world.new_entity((3usize, 4u32));
        world.run::<(&mut usize, ThreadPool), _>(|(usizes, thread_pool)| {
            thread_pool.install(|| {
                let sum: usize = (&usizes,).par_iter().cloned().sum();
                assert_eq!(sum, 4);
            });
        });
    }
    #[cfg(feature = "parallel")]
    #[test]
    fn packed_parallel_iterator() {
        use rayon::prelude::*;

        let world = World::new::<(usize, u32)>();
        world.pack_owned::<(usize, u32)>();
        world.new_entity((0usize, 1u32));
        world.new_entity((2usize, 3u32));
        world.run::<(&mut usize, &u32, ThreadPool), _>(|(mut usizes, u32s, thread_pool)| {
            let counter = std::sync::atomic::AtomicUsize::new(0);
            thread_pool.install(|| {
                if let ParIter::Packed(iter) = (&mut usizes, &u32s).par_iter() {
                    iter.for_each(|(x, y)| {
                        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        *x += *y as usize;
                    });
                } else {
                    panic!()
                }
            });
            assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
            let mut iter = usizes.iter();
            assert_eq!(iter.next(), Some(&mut 1));
            assert_eq!(iter.next(), Some(&mut 5));
            assert_eq!(iter.next(), None);
        });
    }
    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_iterator() {
        use rayon::prelude::*;

        let world = World::new::<(usize, u32)>();
        world.new_entity((0usize, 1u32));
        world.new_entity((2usize, 3u32));
        world.run::<(&mut usize, &u32, ThreadPool), _>(|(mut usizes, u32s, thread_pool)| {
            let counter = std::sync::atomic::AtomicUsize::new(0);
            thread_pool.install(|| {
                (&mut usizes, &u32s).par_iter().for_each(|(x, y)| {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    *x += *y as usize;
                });
            });
            assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
            let mut iter = usizes.iter();
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
            fn run(&self, _: <Self::Data as SystemData>::View) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }

        let world = World::new::<(usize, u32)>();
        world.add_workload("default", (System1,));

        rayon::scope(|s| {
            s.spawn(|_| world.run_default());
            s.spawn(|_| world.run_default());
        });
    }
    #[cfg(feature = "parallel")]
    #[test]
    #[should_panic(
        expected = "Result::unwrap()` on an `Err` value: Cannot mutably borrow while already borrowed."
    )]
    fn two_bad_workloads() {
        struct System1;
        impl<'a> System<'a> for System1 {
            type Data = (&'a mut usize,);
            fn run(&self, _: <Self::Data as SystemData>::View) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }

        let world = World::new::<(usize, u32)>();
        world.add_workload("default", (System1,));

        rayon::scope(|s| {
            s.spawn(|_| world.run_default());
            s.spawn(|_| world.run_default());
        });
    }
    #[test]
    #[should_panic(expected = "Entity has to be alive to add component to it.")]
    fn add_component_with_old_key() {
        let world = World::new::<(usize, u32)>();
        let entity = world.new_entity((0usize, 1u32));
        world.delete(entity);

        world.run::<(Entities, &mut usize, &mut u32), _>(|(entities, mut usizes, mut u32s)| {
            entities.add_component((&mut usizes, &mut u32s), (1, 2), entity);
        });
    }
    #[test]
    fn remove_component_with_old_key() {
        let world = World::new::<(usize, u32)>();
        let entity = world.new_entity((0usize, 1u32));
        world.delete(entity);
        world.new_entity((1usize, 2u32));

        world.run::<(&mut usize, &mut u32), _>(|(mut usizes, mut u32s)| {
            let (old_usize, old_u32) =
                Remove::<(usize, u32)>::remove((&mut usizes, &mut u32s), entity);
            assert!(old_usize.is_none() && old_u32.is_none());
        });
    }
}
