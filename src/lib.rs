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
mod run;
mod sparse_array;
mod world;

pub use add_component::AddComponent;
pub use add_entity::AddEntity;
pub use entity::Entities;
pub use get::GetComponent;
pub use iterators::IntoIter;
pub use not::Not;
pub use run::Run;
pub use world::World;

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
        (&mut *usizes, &mut *u32s).add_component((0usize, 1u32), entity1);
        (&mut usizes, &mut u32s).add_component((2usize, 3u32), entity1);
        (usizes, u32s).add_component((4usize, 5u32), entity1);
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
            let mut iter1 = (&usizes).into_iter();
            let _iter2 = (&usizes).into_iter();
            assert_eq!(iter1.next(), Some(&0));

            // impossible to borrow twice as mutable
            // if switched, the next two lines should trigger an error
            let _iter = (&mut usizes).into_iter();
            let mut iter = (&mut usizes).into_iter();
            assert_eq!(iter.next(), Some(&mut 0));
            assert_eq!(iter.next(), Some(&mut 2));
            assert_eq!(iter.next(), None);

            // possible to borrow twice as immutable
            let mut iter = (&usizes, &u32s).into_iter();
            let _iter = (&usizes, &u32s).into_iter();
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), Some((&2, &3)));
            assert_eq!(iter.next(), None);

            // impossible to borrow twice as mutable
            // if switched, the next two lines should trigger an error
            let _iter = (&mut usizes, &u32s).into_iter();
            let mut iter = (&mut usizes, &u32s).into_iter();
            assert_eq!(iter.next(), Some((&mut 0, &1)));
            assert_eq!(iter.next(), Some((&mut 2, &3)));
            assert_eq!(iter.next(), None);
        });
    }
    #[test]
    fn iterators() {
        let world = World::new::<(usize, u32)>();
        world.run::<(Entities, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
            entities.add((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add((&mut usizes,), (2usize,));

            let mut iter1 = (&usizes).into_iter();
            assert_eq!(iter1.next(), Some(&0));
            assert_eq!(iter1.next(), Some(&2));
            assert_eq!(iter1.next(), None);

            let mut iter1 = (&usizes, &u32s).into_iter();
            assert_eq!(iter1.next(), Some((&0, &1)));
            assert_eq!(iter1.next(), None);
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
}
