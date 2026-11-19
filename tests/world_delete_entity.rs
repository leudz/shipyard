use core::any::type_name;
use shipyard::error;
use shipyard::*;
use std::sync::{Arc, Mutex};

#[derive(PartialEq, Eq, Debug)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();

    let entity1 = world.add_entity((USIZE(0), U32(1)));
    let entity2 = world.add_entity((USIZE(2), U32(3)));

    assert!(world.delete_entity(entity1));
    assert!(!world.delete_entity(entity1));

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
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::new();

    world.borrow::<ViewMut<USIZE>>().unwrap().track_all();

    let entity1 = world.add_entity((USIZE(0),));
    let entity2 = world.add_entity((USIZE(2),));

    assert!(world.delete_entity(entity1));
    assert!(!world.delete_entity(entity1));

    let usizes = world.borrow::<ViewMut<USIZE, track::All>>().unwrap();
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

/// Asserts that deleting an entity at generation 65,533 removes it from iteration and reports its
/// original ID once, even if deletion is attempted again.
#[test]
fn delete_exhausted() {
    let mut world = World::new();
    let entity = EntityId::new_from_index_and_gen(0, 65_533);
    assert!(world.spawn(entity));

    let deleted = Arc::new(Mutex::new(Vec::new()));
    let recorder = deleted.clone();
    world.on_deletion(move |id| recorder.lock().unwrap().push(id));

    assert!(world.delete_entity(entity));
    assert!(!world.delete_entity(entity));
    assert_eq!(world.borrow::<EntitiesView>().unwrap().iter().next(), None);
    assert_eq!(*deleted.lock().unwrap(), [entity]);
}

/// Asserts that deleting grouped entities leaves the remaining pairs yielded with their values,
/// before and after a regroup.
#[test]
fn grouped() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let mut world = World::builder().with_group::<(USIZE, U32)>().build();
    let entities: Vec<EntityId> = (0..6)
        .map(|i| world.add_entity((USIZE(i), U32(i as u32))))
        .collect();
    world.regroup();

    assert!(world.delete_entity(entities[2]));
    assert!(world.delete_entity(entities[5]));

    let pairs = |world: &World| {
        world.run(|(usizes, u32s): (View<USIZE>, View<U32>)| {
            let mut pairs: Vec<_> = (&usizes, &u32s)
                .iter()
                .with_id()
                .map(|(id, (usize, u32))| (id, usize.0, u32.0))
                .collect();
            pairs.sort_unstable();
            pairs
        })
    };
    let expected = [0, 1, 3, 4].map(|i| (entities[i], i, i as u32));
    assert_eq!(pairs(&world), expected);

    world.regroup();
    assert_eq!(pairs(&world), expected);
}
