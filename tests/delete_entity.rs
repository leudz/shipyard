use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    drop((entities, usizes, u32s));

    let mut all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();
    assert!(all_storages.delete_entity(entity1));
    assert!(!all_storages.delete_entity(entity1));
    drop(all_storages);

    let (usizes, u32s) = world.try_borrow::<(View<usize>, View<u32>)>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(
        (&u32s).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<u32>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(u32s.get(entity2), Ok(&3));
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();

    usizes.update_pack();
    let entity1 = entities.add_entity(&mut usizes, 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    drop((entities, usizes));

    let mut all_storages = world.try_borrow::<AllStoragesViewMut>().unwrap();
    assert!(all_storages.delete_entity(entity1));
    assert!(!all_storages.delete_entity(entity1));
    drop(all_storages);

    let mut usizes = world.try_borrow::<ViewMut<usize>>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(usizes.try_deleted().unwrap().len(), 1);
    assert_eq!(usizes.try_take_deleted().unwrap(), vec![(entity1, 0)]);
    assert_eq!(usizes.try_removed().unwrap().len(), 0);
}
