#![deny(bare_trait_objects)]

mod add_component;
mod add_entity;
mod atomic_refcell;
mod component_storage;
mod entity;
mod error;
mod get;
mod get_storage;
mod iterators;
mod not;
mod pack;
mod remove;
mod run;
mod sparse_array;
mod world;

pub use add_component::AddComponent;
pub use add_entity::AddEntity;
pub use entity::Entities;
pub use get::GetComponent;
pub use iterators::{IntoIter, Iter2, Iter3, Iter4, Iter5};
pub use not::Not;
pub use pack::OwnedPack;
pub use remove::Remove;
pub use run::Run;
pub use run::{System, SystemData};
pub use world::World;

/// Type used to borrow the rayon::ThreadPool inside `World`.
pub struct ThreadPool;

#[cfg(test)]
mod test {
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
        let entity1 = (&mut usizes, &mut u32s).add_entity((0, 1), &mut entities);
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
    }
    #[test]
    fn add_component() {
        let world = World::default();
        world.register::<usize>();
        world.register::<u32>();
        let mut entities = world.entities_mut();
        let (mut usizes, mut u32s) = world.get_storage::<(&mut usize, &mut u32)>();
        let entity1 = ().add_entity((), &mut entities);
        (&mut *usizes, &mut *u32s).add_component((0, 1), entity1);
        (&mut usizes, &mut u32s).add_component((2, 3), entity1);
        (usizes, u32s).add_component((4, 5), entity1);
        let storages = world.get_storage::<(&usize, &u32)>();
        assert_eq!(storages.get(entity1).unwrap(), (&4, &5));
    }
    #[test]
    fn run() {
        let world = World::new::<(usize, u32)>();
        world.run::<(Entities, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
            entities.add((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add((&mut usizes, &mut u32s), (2usize, 3u32));

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
        });
    }
    #[test]
    fn iterators() {
        let world = World::new::<(usize, u32)>();
        world.run::<(Entities, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
            let entity1 = entities.add((&mut usizes,), (0usize,));
            (&mut u32s,).add_component((1u32,), entity1);
            entities.add((&mut usizes,), (2usize,));

            let mut iter = (&usizes).iter();
            assert_eq!(iter.next(), Some(&0));
            assert_eq!(iter.next(), Some(&2));
            assert_eq!(iter.next(), None);

            let mut iter = (&usizes, &u32s).iter();
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), None);
        });
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

        (&mut usizes, &mut u32s).add_component((0, 1), entity);
        (&mut usizes, &mut u32s).add_component((2,), entity);
    }
    #[test]
    #[should_panic(
        expected = "called `Result::unwrap()` on an `Err` value: MissingPackStorage(TypeId { t: 8766594652559642870 })"
    )]
    fn pack_missing_storage() {
        assert_eq!(
            format!("{:?}", std::any::TypeId::of::<usize>()),
            "TypeId { t: 8766594652559642870 }"
        );

        let world = World::new::<(usize, u32)>();
        let entity = world.new_entity(());
        world.pack_owned::<(usize, u32)>();
        let (mut usizes,) = world.get_storage::<(&mut usize,)>();

        (&mut usizes,).add_component((0,), entity);
    }
    #[test]
    fn pack_iterator() {
        let world = World::new::<(usize, u32)>();
        world.pack_owned::<(usize, u32)>();
        world.new_entity((0usize, 1u32));
        world.new_entity((2usize,));
        world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
            if let Iter2::Packed(mut iter) = (usizes, u32s).iter() {
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
            if let Iter2::Packed(iter) = (&usizes, &u32s).iter() {
                let mut iter = iter.into_chunk(2);
                assert_eq!(iter.next(), Some((&[0, 2][..], &[1, 3][..])));
                assert_eq!(iter.next(), Some((&[4, 6][..], &[5, 7][..])));
                assert_eq!(iter.next(), Some((&[8][..], &[9][..])));
                assert_eq!(iter.next(), None);
            } else {
                panic!("not packed");
            }
            if let Iter2::Packed(iter) = (&usizes, &u32s).iter() {
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
            if let Iter2::Packed(mut iter) = (&usizes, &u32s).iter() {
                iter.next();
                let mut iter = iter.into_chunk(2);
                assert_eq!(iter.next(), Some((&[2, 4][..], &[3, 5][..])));
                assert_eq!(iter.next(), Some((&[6, 8][..], &[7, 9][..])));
                assert_eq!(iter.next(), None);
            } else {
                panic!("not packed");
            }
            if let Iter2::Packed(mut iter) = (&usizes, &u32s).iter() {
                iter.next();
                let mut iter = iter.into_chunk_exact(2);
                assert_eq!(iter.next(), Some((&[2, 4][..], &[3, 5][..])));
                assert_eq!(iter.next(), Some((&[6, 8][..], &[7, 9][..])));
                assert_eq!(iter.remainder(), (&[][..], &[][..]));
                assert_eq!(iter.next(), None);
            } else {
                panic!("not packed");
            }
            if let Iter2::Packed(mut iter) = (&usizes, &u32s).iter() {
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
            if let Iter2::Packed(mut iter) = (&usizes, &u32s).iter() {
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
        assert_eq!(usizes.get(entity2), Some(&mut 2));
        assert_eq!(u32s.get(entity2), Some(&mut 3));
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
        assert_eq!(usizes.get(entity2), Some(&mut 2));
        assert_eq!(u32s.get(entity2), Some(&mut 3));
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
}
