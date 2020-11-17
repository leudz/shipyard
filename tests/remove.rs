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
    let component = usizes.remove(entity1);
    assert_eq!(component, Some(0usize));
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
    let component = usizes.remove(entity1);
    assert_eq!(component, Some(0));
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
    assert_eq!(usizes.try_deleted().unwrap().len(), 0);
    assert_eq!(usizes.try_removed().unwrap(), &[entity1]);

    let mut iter = usizes.try_removed_or_deleted().unwrap();
    assert_eq!(iter.next(), Some(entity1));
    assert_eq!(iter.next(), None);

    drop(iter);

    usizes.delete(entity2);

    let mut iter = usizes.try_removed_or_deleted().unwrap();
    assert_eq!(iter.next(), Some(entity1));
    assert_eq!(iter.next(), Some(entity2));
    assert_eq!(iter.next(), None);
}

#[test]
fn old_key() {
    let world = World::new();

    let entity = world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| { entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)) },
        )
        .unwrap();

    world
        .try_run(|mut all_storages: AllStoragesViewMut| {
            all_storages.delete_entity(entity);
        })
        .unwrap();

    world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
                let (old_usize, old_u32) = (&mut usizes, &mut u32s).remove(entity);
                assert!(old_usize.is_none() && old_u32.is_none());
            },
        )
        .unwrap();
}

#[test]
fn newer_key() {
    let world = World::new();

    world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                let entity = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));

                entities.delete_unchecked(entity);
                assert_eq!(usizes.len(), 1);
                assert_eq!(u32s.len(), 1);
                let new_entity = entities.add_entity((), ());
                let (old_usize, old_u32) = (&mut usizes, &mut u32s).remove(new_entity);

                assert_eq!(old_usize, None);
                assert_eq!(old_u32, None);
                assert_eq!(usizes.len(), 0);
                assert_eq!(u32s.len(), 0);
            },
        )
        .unwrap();
}
