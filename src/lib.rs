#![deny(bare_trait_objects)]

mod add_component;
mod add_entity;
mod atomic_refcell;
mod component_storage;
mod entity;
mod error;
mod get;
mod get_storage;
mod run;
mod sparse_array;
mod world;
mod not;

pub use add_component::AddComponent;
pub use add_entity::AddEntity;
pub use get::GetComponent;
pub use run::Run;
pub use world::World;
pub use not::Not;

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
        let world = World::default();
        world.new_entity((0usize, 1u32));
        world.new_entity((2usize, 3u32));
        world.run::<(&mut usize, &u32), _>(|(usizes, u32s)| {});
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
