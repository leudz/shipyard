use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>().unwrap();

    entities.add_entity(&mut u32s, 0);
    entities.add_entity(&mut u32s, 1);
    entities.add_entity(&mut u32s, 2);

    drop((entities, u32s));
    world.borrow::<AllStoragesViewMut>().unwrap().clear();

    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>().unwrap();

    assert_eq!(u32s.len(), 0);
    let entity0 = entities.add_entity(&mut u32s, 3);
    let entity1 = entities.add_entity(&mut u32s, 4);
    let entity2 = entities.add_entity(&mut u32s, 5);
    let entity3 = entities.add_entity(&mut u32s, 5);

    assert_eq!("EntityId { index: 0, gen: 1 }", format!("{:?}", entity0));
    assert_eq!("EntityId { index: 1, gen: 1 }", format!("{:?}", entity1));
    assert_eq!("EntityId { index: 2, gen: 1 }", format!("{:?}", entity2));
    assert_eq!("EntityId { index: 3, gen: 0 }", format!("{:?}", entity3));
}

#[test]
fn update() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>().unwrap();

    usizes.track_all();
    let entity1 = entities.add_entity(&mut usizes, 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    drop((entities, usizes));

    let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    all_storages.clear();
    drop(all_storages);

    let mut usizes = world.borrow::<ViewMut<usize>>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(
        usizes.get(entity2),
        Err(error::MissingComponent {
            id: entity2,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(usizes.deleted().len(), 2);
    assert_eq!(usizes.take_deleted(), vec![(entity1, 0), (entity2, 2)]);
    assert_eq!(usizes.len(), 0);
}
