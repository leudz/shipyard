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
    let world = World::new();
    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();

    usizes.update_pack();

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
    assert_eq!(usizes.try_deleted().unwrap().len(), 1);
    assert_eq!(usizes.try_take_deleted().unwrap(), vec![(entity1, 0)]);
}

#[test]
fn strip() {
    let world = World::new();

    let (entity1, entity2) = world
        .try_run(
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
        .try_run(|mut all_storages: AllStoragesViewMut| {
            all_storages.strip(entity1);
        })
        .unwrap();

    world
        .try_run(|(mut usizes, mut u32s): (ViewMut<usize>, ViewMut<u32>)| {
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
        .try_run(|mut all_storages: AllStoragesViewMut| {
            assert!(all_storages.delete(entity1));
        })
        .unwrap();
}

#[test]
fn strip_except() {
    let world = World::new();

    let (entity1, entity2) = world
        .try_run(
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
        .try_run(|mut all_storages: AllStoragesViewMut| {
            all_storages.strip_except::<SparseSet<u32>>(entity1);
        })
        .unwrap();

    world
        .try_run(|(mut usizes, u32s): (ViewMut<usize>, ViewMut<u32>)| {
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
        .try_run(|mut all_storages: AllStoragesViewMut| {
            assert!(all_storages.delete(entity1));
        })
        .unwrap();
}
