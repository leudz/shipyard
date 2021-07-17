use core::any::type_name;
use shipyard::error;
use shipyard::*;

#[derive(Debug, PartialEq, Eq)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Nothing;
}

#[test]
fn no_pack() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Nothing;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>()
        .unwrap();

    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
    let entity2 = entities.add_entity((&mut usizes, &mut u32s), (USIZE(2), U32(3)));
    drop((entities, usizes, u32s));

    let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    assert!(all_storages.delete_entity(entity1));
    assert!(!all_storages.delete_entity(entity1));
    drop(all_storages);

    let (usizes, u32s) = world.borrow::<(View<USIZE>, View<U32>)>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(
        (&u32s).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<U32>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&USIZE(2)));
    assert_eq!(u32s.get(entity2), Ok(&U32(3)));
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

    let mut usizes = world.borrow::<ViewMut<USIZE>>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&USIZE(2)));
    assert_eq!(usizes.removed().len(), 1);
    assert_eq!(usizes.take_removed(), vec![entity1]);
}
