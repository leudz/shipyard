use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    usizes.delete(entity1);
    assert_eq!(
        (&mut usizes).get(entity1).err(),
        Some(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(*(&mut u32s).get(entity1).unwrap(), 1);
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(u32s.get(entity2), Ok(&3));
}

#[test]
fn update() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<usize>)>().unwrap();

    usizes.track_all();

    let entity1 = entities.add_entity(&mut usizes, 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    usizes.delete(entity1);
    assert_eq!(
        usizes.get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(usizes.len(), 1);
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes.modified().iter().count(), 0);
    assert_eq!(usizes.deleted(), &[(entity1, 0)]);

    let mut iter = usizes.removed_or_deleted();
    assert_eq!(iter.next(), Some(entity1));
    assert_eq!(iter.next(), None);

    drop(iter);

    assert_eq!(usizes.take_deleted(), vec![(entity1, 0)]);
}

#[test]
fn strip() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (entity1, entity2) = world
        .run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                (
                    entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)),
                    entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)),
                )
            },
        )
        .unwrap();

    world
        .run(|mut all_storages: AllStoragesViewMut| {
            all_storages.strip(entity1);
        })
        .unwrap();

    world
        .run(|(mut usizes, mut u32s): (ViewMut<usize>, ViewMut<u32>)| {
            assert_eq!(
                (&mut usizes).get(entity1).err(),
                Some(error::MissingComponent {
                    id: entity1,
                    name: type_name::<usize>(),
                })
            );
            assert_eq!(
                (&mut u32s).get(entity1).err(),
                Some(error::MissingComponent {
                    id: entity1,
                    name: type_name::<u32>(),
                })
            );
            assert_eq!(usizes.get(entity2), Ok(&2));
            assert_eq!(u32s.get(entity2), Ok(&3));
        })
        .unwrap();

    world
        .run(|mut all_storages: AllStoragesViewMut| {
            assert!(all_storages.delete_entity(entity1));
        })
        .unwrap();
}

#[test]
fn retain() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (entity1, entity2) = world
        .run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                (
                    entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)),
                    entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)),
                )
            },
        )
        .unwrap();

    world
        .run(|mut all_storages: AllStoragesViewMut| {
            all_storages.retain::<SparseSet<u32>>(entity1);
        })
        .unwrap();

    world
        .run(|(mut usizes, u32s): (ViewMut<usize>, ViewMut<u32>)| {
            assert_eq!(
                (&mut usizes).get(entity1).err(),
                Some(error::MissingComponent {
                    id: entity1,
                    name: type_name::<usize>(),
                })
            );
            assert_eq!((&u32s).get(entity1), Ok(&1));
            assert_eq!(usizes.get(entity2), Ok(&2));
            assert_eq!(u32s.get(entity2), Ok(&3));
        })
        .unwrap();

    world
        .run(|mut all_storages: AllStoragesViewMut| {
            assert!(all_storages.delete_entity(entity1));
        })
        .unwrap();
}
