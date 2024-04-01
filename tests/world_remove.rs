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

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity1 = world.add_entity((USIZE(0), U64(1u64)));
    let entity2 = world.add_entity((USIZE(2), U64(3u64)));

    let (component,) = world.remove::<(USIZE,)>(entity1);
    assert_eq!(component, Some(USIZE(0)));

    let (usizes, u64s) = world.borrow::<(View<USIZE>, View<U64>)>().unwrap();
    assert_eq!(
        (&usizes).get(entity1).err(),
        Some(error::MissingComponent {
            id: entity1,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(u64s.get(entity1), Ok(&U64(1)));
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

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity1 = world.add_entity((USIZE(0usize),));
    let entity2 = world.add_entity((USIZE(2usize),));

    let (component,) = world.remove::<(USIZE,)>(entity1);
    assert_eq!(component, Some(USIZE(0)));

    world.run(|usizes: View<USIZE>| {
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
    });

    world.remove::<(USIZE,)>(entity2);

    world.run(|usizes: View<USIZE>| {
        assert_eq!(usizes.removed().collect::<Vec<_>>(), vec![entity1, entity2]);
    });
}

#[test]
fn old_key() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity = world.add_entity((USIZE(0), U64(1)));
    world.delete_entity(entity);

    world.add_entity((USIZE(2), U64(3)));

    let (old_usize, old_u64) = world.remove::<(USIZE, U64)>(entity);
    assert!(old_usize.is_none() && old_u64.is_none());
}

#[test]
fn newer_key() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity = world.add_entity((USIZE(0), U64(1)));

    world
        .borrow::<EntitiesViewMut>()
        .unwrap()
        .delete_unchecked(entity);

    world.run(|(usizes, u64s): (ViewMut<USIZE>, ViewMut<U64>)| {
        assert_eq!(usizes.len(), 1);
        assert_eq!(u64s.len(), 1);
    });

    let new_entity = world.add_entity(());
    let (old_usize, old_u64) = world.remove::<(USIZE, U64)>(new_entity);

    assert_eq!(old_usize, None);
    assert_eq!(old_u64, None);

    world.run(|(usizes, u64s): (ViewMut<USIZE>, ViewMut<U64>)| {
        assert_eq!(usizes.len(), 0);
        assert_eq!(u64s.len(), 0);
    });
}
