use core::any::type_name;
use shipyard::error;
use shipyard::*;

struct U64(u64);
impl Component for U64 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u64s) = world.borrow::<(EntitiesViewMut, ViewMut<U64>)>().unwrap();

    entities.add_entity(&mut u64s, U64(0));
    entities.add_entity(&mut u64s, U64(1));
    entities.add_entity(&mut u64s, U64(2));

    drop((entities, u64s));
    world.borrow::<AllStoragesViewMut>().unwrap().clear();

    let (mut entities, mut u64s) = world.borrow::<(EntitiesViewMut, ViewMut<U64>)>().unwrap();

    assert_eq!(u64s.len(), 0);
    let entity0 = entities.add_entity(&mut u64s, U64(3));
    let entity1 = entities.add_entity(&mut u64s, U64(4));
    let entity2 = entities.add_entity(&mut u64s, U64(5));
    let entity3 = entities.add_entity(&mut u64s, U64(5));

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
        type Tracking = track::All;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut usizes) = world.borrow::<(EntitiesViewMut, ViewMut<USIZE>)>().unwrap();

    let entity1 = entities.add_entity(&mut usizes, USIZE(0));
    let entity2 = entities.add_entity(&mut usizes, USIZE(2));
    drop((entities, usizes));

    let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();
    all_storages.clear();
    drop(all_storages);

    let usizes = world.borrow::<ViewMut<USIZE>>().unwrap();
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
        type Tracking = track::All;
    }

    fn system(u64s: View<U64>, mut usizes: ViewMut<USIZE>) {
        usizes.clear();

        for id in u64s.iter().ids() {
            usizes.add_component_unchecked(id, USIZE(0));
        }

        assert_eq!(usizes.inserted().iter().count(), 1);
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.add_entity((U64(0),));

    Workload::new("")
        .with_system(system)
        .add_to_world(&world)
        .unwrap();

    world.run_default().unwrap();
    world.run_default().unwrap();
    world.run_default().unwrap();
}
