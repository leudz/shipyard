use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[test]
fn no_pack() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity1 = world.add_entity((0usize, 1u32));
    let entity2 = world.add_entity((2usize, 3u32));

    let (component,) = world.remove::<(usize,)>(entity1);
    assert_eq!(component, Some(0usize));

    let (usizes, u32s) = world.borrow::<(View<usize>, View<u32>)>().unwrap();
    assert_eq!(
        (&usizes).get(entity1).err(),
        Some(error::MissingComponent {
            id: entity1,
            name: type_name::<usize>(),
        })
    );
    assert_eq!(u32s.get(entity1), Ok(&1));
    assert_eq!(usizes.get(entity2), Ok(&2));
    assert_eq!(u32s.get(entity2), Ok(&3));
}

#[test]
fn update() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.borrow::<ViewMut<usize>>().unwrap().track_all();

    let entity1 = world.add_entity((0usize,));
    let entity2 = world.add_entity((2usize,));

    let (component,) = world.remove::<(usize,)>(entity1);
    assert_eq!(component, Some(0));

    world
        .run(|usizes: View<usize>| {
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
            assert_eq!(usizes.deleted().len(), 0);
            assert_eq!(usizes.removed(), &[entity1]);

            let mut iter = usizes.removed_or_deleted();
            assert_eq!(iter.next(), Some(entity1));
            assert_eq!(iter.next(), None);

            drop(iter);
        })
        .unwrap();

    world.delete_component::<(usize,)>(entity2);

    world
        .run(|usizes: View<usize>| {
            let mut iter = usizes.removed_or_deleted();
            assert_eq!(iter.next(), Some(entity1));
            assert_eq!(iter.next(), Some(entity2));
            assert_eq!(iter.next(), None);
        })
        .unwrap();
}

#[test]
fn old_key() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity = world.add_entity((0usize, 1u32));
    world.delete_entity(entity);

    world.add_entity((2usize, 3u32));

    let (old_usize, old_u32) = world.remove::<(usize, u32)>(entity);
    assert!(old_usize.is_none() && old_u32.is_none());
}

#[test]
fn newer_key() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity = world.add_entity((0usize, 1u32));

    world
        .borrow::<EntitiesViewMut>()
        .unwrap()
        .delete_unchecked(entity);

    world
        .run(|(usizes, u32s): (ViewMut<usize>, ViewMut<u32>)| {
            assert_eq!(usizes.len(), 1);
            assert_eq!(u32s.len(), 1);
        })
        .unwrap();

    let new_entity = world.add_entity(());
    let (old_usize, old_u32) = world.remove::<(usize, u32)>(new_entity);

    assert_eq!(old_usize, None);
    assert_eq!(old_u32, None);

    world
        .run(|(usizes, u32s): (ViewMut<usize>, ViewMut<u32>)| {
            assert_eq!(usizes.len(), 0);
            assert_eq!(u32s.len(), 0);
        })
        .unwrap();
}
