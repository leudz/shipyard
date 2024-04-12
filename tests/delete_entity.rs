use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[derive(Debug, PartialEq, Eq)]
struct U64(u64);
impl Component for U64 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    #[derive(Debug, PartialEq, Eq)]
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
    drop((entities, usizes, u64s));

    let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    assert!(all_storages.delete_entity(entity1));
    assert!(!all_storages.delete_entity(entity1));
    drop(all_storages);

    let (usizes, u64s) = world.borrow::<(View<USIZE>, View<U64>)>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(
        (&u64s).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<U64>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&USIZE(2)));
    assert_eq!(u64s.get(entity2), Ok(&U64(3)));
}

#[test]
fn update() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>)>().unwrap();

    let entity1 = entities.add_entity(&mut usizes, USIZE(0));
    let entity2 = entities.add_entity(&mut usizes, USIZE(2));
    drop((entities, usizes));

    let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    assert!(all_storages.delete_entity(entity1));
    assert!(!all_storages.delete_entity(entity1));
    drop(all_storages);

    let usizes = world.borrow::<ViewMut<USIZE>>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&USIZE(2)));
    assert_eq!(usizes.removed().count(), 0);
    assert_eq!(
        usizes.deleted().collect::<Vec<_>>(),
        vec![(entity1, &USIZE(0))]
    );
}
