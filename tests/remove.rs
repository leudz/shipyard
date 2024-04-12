use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[derive(PartialEq, Eq, Debug)]
struct U64(u64);
impl Component for U64 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes, mut u64s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U64>)>()
        .unwrap();

    let entity1 = entities.add_entity((&mut usizes, &mut u64s), (USIZE(0), U64(1)));
    let entity2 = entities.add_entity((&mut usizes, &mut u64s), (USIZE(2), U64(3)));
    let component = usizes.remove(entity1);
    assert_eq!(component, Some(USIZE(0)));
    assert_eq!(
        (&mut usizes).get(entity1).err(),
        Some(error::MissingComponent {
            id: entity1,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(*(&mut u64s).get(entity1).unwrap(), U64(1));
    assert_eq!(usizes.get(entity2), Ok(&USIZE(2)));
    assert_eq!(u64s.get(entity2), Ok(&U64(3)));
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>)>().unwrap();

    let entity1 = entities.add_entity(&mut usizes, USIZE(0));
    let entity2 = entities.add_entity(&mut usizes, USIZE(2));
    let component = usizes.remove(entity1);
    assert_eq!(component, Some(USIZE(0)));
    assert_eq!(
        usizes.get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&USIZE(2)));
    assert_eq!(usizes.len(), 1);
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes.modified().iter().count(), 0);
    assert_eq!(usizes.removed().collect::<Vec<_>>(), vec![entity1]);

    usizes.remove(entity2);

    let mut iter = usizes.removed();
    assert_eq!(iter.next(), Some(entity1));
    assert_eq!(iter.next(), Some(entity2));
    assert_eq!(iter.next(), None);
}

#[test]
fn old_key() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity = world.run(
        |(mut entities, mut usizes, mut u64s): (EntitiesViewMut, ViewMut<USIZE>, ViewMut<U64>)| {
            entities.add_entity((&mut usizes, &mut u64s), (USIZE(0), U64(1)))
        },
    );

    world.run(|mut all_storages: AllStoragesViewMut| {
        all_storages.delete_entity(entity);
    });

    world.run(
        |(mut entities, mut usizes, mut u64s): (EntitiesViewMut, ViewMut<USIZE>, ViewMut<U64>)| {
            entities.add_entity((&mut usizes, &mut u64s), (USIZE(2), U64(3)));
            let (old_usize, old_u64) = (&mut usizes, &mut u64s).remove(entity);
            assert!(old_usize.is_none() && old_u64.is_none());
        },
    );
}

#[test]
fn newer_key() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.run(
        |(mut entities, mut usizes, mut u64s): (EntitiesViewMut, ViewMut<USIZE>, ViewMut<U64>)| {
            let entity = entities.add_entity((&mut usizes, &mut u64s), (USIZE(0), U64(1)));

            entities.delete_unchecked(entity);
            assert_eq!(usizes.len(), 1);
            assert_eq!(u64s.len(), 1);
            let new_entity = entities.add_entity((), ());
            let (old_usize, old_u64) = (&mut usizes, &mut u64s).remove(new_entity);

            assert_eq!(old_usize, None);
            assert_eq!(old_u64, None);
            assert_eq!(usizes.len(), 0);
            assert_eq!(u64s.len(), 0);
        },
    );
}

#[test]
fn track_reset_with_timestamp() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity1 = world.add_entity((USIZE(0),));
    world.remove::<(USIZE,)>(entity1);

    let time = world.get_tracking_timestamp();

    let entity2 = world.add_entity((USIZE(1),));
    world.remove::<(USIZE,)>(entity2);

    let usizes = world.borrow::<View<USIZE>>().unwrap();
    assert_eq!(usizes.removed().collect::<Vec<_>>(), vec![entity1, entity2]);
    drop(usizes);

    world.clear_all_removed_or_deleted_older_than_timestamp(time);

    let usizes = world.borrow::<View<USIZE>>().unwrap();
    assert_eq!(usizes.removed().collect::<Vec<_>>(), vec![entity2]);
    drop(usizes);

    world.clear_all_removed_or_deleted();

    let usizes = world.borrow::<View<USIZE>>().unwrap();
    assert_eq!(usizes.removed().collect::<Vec<_>>(), vec![]);
}
