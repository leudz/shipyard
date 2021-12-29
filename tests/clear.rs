use core::any::type_name;
use shipyard::error;
use shipyard::*;

struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::Untracked;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<U32>)>().unwrap();

    entities.add_entity(&mut u32s, U32(0));
    entities.add_entity(&mut u32s, U32(1));
    entities.add_entity(&mut u32s, U32(2));

    drop((entities, u32s));
    world.borrow::<AllStoragesViewMut>().unwrap().clear();

    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<U32>)>().unwrap();

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

    let mut usizes = world.borrow::<ViewMut<USIZE>>().unwrap();
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
    assert_eq!(usizes.removed().len(), 2);
    assert_eq!(usizes.take_removed(), vec![entity1, entity2]);
    assert_eq!(usizes.len(), 0);
}

#[test]
fn inserted() {
    #[derive(PartialEq, Eq, Debug)]
    struct USIZE(usize);
    impl Component for USIZE {
        type Tracking = track::All;
    }

    fn system(u32s: View<U32>, mut usizes: ViewMut<USIZE>) {
        usizes.clear();

        for id in u32s.iter().ids() {
            usizes.add_component_unchecked(id, USIZE(0));
        }

        assert_eq!(usizes.inserted().iter().count(), 1);
    }

    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.add_entity((U32(0),));

    Workload::builder("")
        .with_system(system)
        .add_to_world(&world)
        .unwrap();

    world.run_default().unwrap();
    world.run_default().unwrap();
    world.run_default().unwrap();
}
