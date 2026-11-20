use core::any::type_name;
use shipyard::error;
use shipyard::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

#[allow(unused)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    #[allow(unused)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let world = World::new();

    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<U32>)>().unwrap();

    let e0 = entities.add_entity(&mut u32s, U32(0));
    let e1 = entities.add_entity(&mut u32s, U32(1));
    let e2 = entities.add_entity(&mut u32s, U32(2));

    drop((entities, u32s));
    world.borrow::<AllStoragesViewMut>().unwrap().clear();

    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<U32>)>().unwrap();

    assert!(u32s.get(e0).is_err());
    assert!(u32s.get(e1).is_err());
    assert!(u32s.get(e2).is_err());
    assert!(!entities.is_alive(e0));
    assert!(!entities.is_alive(e1));
    assert!(!entities.is_alive(e2));
    assert_eq!(entities.iter().count(), 0);

    assert_eq!(u32s.len(), 0);
    let entity0 = entities.add_entity(&mut u32s, U32(3));
    let entity1 = entities.add_entity(&mut u32s, U32(4));
    let entity2 = entities.add_entity(&mut u32s, U32(5));
    let entity3 = entities.add_entity(&mut u32s, U32(5));

    assert_eq!("EId(0.1)", format!("{:?}", entity0));
    assert_eq!("EId(1.1)", format!("{:?}", entity1));
    assert_eq!("EId(2.1)", format!("{:?}", entity2));
    assert_eq!("EId(3.0)", format!("{:?}", entity3));
}

#[test]
fn update() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let world = World::new();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>)>().unwrap();
    usizes.track_all();

    let entity1 = entities.add_entity(&mut usizes, USIZE(0));
    let entity2 = entities.add_entity(&mut usizes, USIZE(2));
    drop((entities, usizes));

    let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    all_storages.clear();
    drop(all_storages);

    let usizes = world.borrow::<ViewMut<USIZE, track::All>>().unwrap();
    assert_eq!(
        (&usizes).get(entity1),
        Err(error::MissingComponent {
            id: entity1,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(
        usizes.get(entity2),
        Err(error::MissingComponent {
            id: entity2,
            name: type_name::<USIZE>(),
        })
    );
    assert_eq!(
        usizes.deleted().collect::<Vec<_>>(),
        vec![(entity1, &USIZE(0)), (entity2, &USIZE(2))]
    );
    assert_eq!(usizes.len(), 0);
}

#[test]
fn inserted() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    fn system(u32s: View<U32>, mut usizes: ViewMut<USIZE, track::All>) {
        usizes.clear();

        for id in u32s.iter().ids() {
            usizes.add_component_unchecked(id, USIZE(0));
        }

        assert_eq!(usizes.inserted().iter().count(), 1);
    }

    let mut world = World::new();

    world.borrow::<ViewMut<USIZE>>().unwrap().track_all();

    world.add_entity((U32(0),));

    Workload::new("")
        .with_system(system)
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
    world.run_default_workload().unwrap();
    world.run_default_workload().unwrap();
}

/// Asserts that `clear` deletes every entity through the world's deletion callback.
#[test]
fn clear_on_deletion() {
    let mut world = World::new();
    let deleted = Arc::new(Mutex::new(Vec::new()));
    let recorder = deleted.clone();
    world.on_deletion(move |entity| recorder.lock().unwrap().push(entity));
    let entity0 = world.add_entity(());
    let entity1 = world.add_entity(());

    world.borrow::<AllStoragesViewMut>().unwrap().clear();

    let mut deleted = deleted.lock().unwrap().clone();
    deleted.sort_unstable();
    assert_eq!(deleted, [entity0, entity1]);
}

/// Asserts that world clear reports each component with its entity and records it as a deletion.
#[test]
fn world_clear_on_removal() {
    let mut world = World::new();
    world.track_all::<U32>();
    let e0 = world.add_entity(U32(4));
    let e1 = world.add_entity(U32(9));
    world.add_entity(());

    let calls = Arc::new(Mutex::new(Vec::new()));
    let recorder = calls.clone();
    world
        .borrow::<ViewMut<U32>>()
        .unwrap()
        .on_removal(move |entity, value| recorder.lock().unwrap().push((entity, value.0)));

    world.clear();
    world.clear();

    assert!(!world.is_entity_alive(e0));
    assert!(!world.is_entity_alive(e1));
    let view = world.borrow::<View<U32, track::All>>().unwrap();
    let expected = [(e0, 4), (e1, 9)];
    assert_eq!(*calls.lock().unwrap(), expected);
    assert_eq!(
        view.deleted()
            .map(|(entity, value)| (entity, value.0))
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(view.removed().count(), 0);
    assert!(view.is_empty());
}

/// Asserts that clearing reports only the original ID of the live entity and that a second clear
/// reports no additional deletions.
#[test]
fn clear_vacant() {
    let mut world = World::new();
    let vacant = world.add_entity(());
    let live = world.add_entity(());
    assert!(world.delete_entity(vacant));

    let deleted = Arc::new(Mutex::new(Vec::new()));
    let recorder = deleted.clone();
    world.on_deletion(move |entity| recorder.lock().unwrap().push(entity));

    world.clear();
    assert_eq!(*deleted.lock().unwrap(), [live]);

    world.clear();
    assert_eq!(*deleted.lock().unwrap(), [live]);
}

/// Asserts that clearing a live entity at generation 65,533 reports its original ID and leaves its
/// slot permanently vacant.
#[test]
fn clear_exhausted() {
    let mut world = World::new();
    let exhausted = EntityId::new_from_index_and_gen(0, 65_533);
    assert!(world.spawn(exhausted));

    let deleted = Arc::new(Mutex::new(Vec::new()));
    let recorder = deleted.clone();
    world.on_deletion(move |id| recorder.lock().unwrap().push(id));

    world.clear();

    assert_eq!(*deleted.lock().unwrap(), [exhausted]);
    assert_eq!(world.borrow::<EntitiesView>().unwrap().iter().next(), None);
    assert_eq!(world.add_entity(()), EntityId::new_from_index_and_gen(1, 0));
}

/// Asserts that a panicking clear callback leaves live IDs and the recycle list intact.
#[test]
fn clear_callback_panic() {
    let mut world = World::new();
    let vacant = world.add_entity(());
    let live = world.add_entity(());
    assert!(world.delete_entity(vacant));
    world.on_deletion(|_| panic!("clear callback panicked"));

    let result = catch_unwind(AssertUnwindSafe(|| world.clear()));

    assert!(result.is_err());
    assert!(world.is_entity_alive(live));
    assert_eq!(world.add_entity(()), EntityId::new_from_index_and_gen(0, 1));
}
