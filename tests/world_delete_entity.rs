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

    let entity1 = world.add_entity((USIZE(0), U64(1)));
    let entity2 = world.add_entity((USIZE(2), U64(3)));

    assert!(world.delete_entity(entity1));
    assert!(!world.delete_entity(entity1));

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
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let entity1 = world.add_entity((USIZE(0),));
    let entity2 = world.add_entity((USIZE(2),));

    assert!(world.delete_entity(entity1));
    assert!(!world.delete_entity(entity1));

    let usizes = world.borrow::<ViewMut<USIZE>>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(usizes.get(entity2), Ok(&USIZE(2)));
    assert_eq!(
        usizes.deleted().collect::<Vec<_>>(),
        vec![(entity1, &USIZE(0))]
    );
    assert_eq!(usizes.removed().count(), 0);
}
