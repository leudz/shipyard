use shipyard::{error::GetStorage, track, Component, IntoIter, View, ViewMut, World};

struct Unit;
impl Component for Unit {
    type Tracking = track::Untracked;
}

struct UnitInsert;
impl Component for UnitInsert {
    type Tracking = track::Insertion;
}

struct UnitInsertAndModification;
impl Component for UnitInsertAndModification {
    type Tracking = track::InsertionAndModification;
}

#[test]
fn runtime_insertion_tracking() {
    let mut world = World::new();

    match world.borrow::<View<Unit, track::Insertion>>() {
        Err(GetStorage::TrackingNotEnabled { .. }) => {}
        _ => panic!("expected an error"),
    };
    match world.borrow::<ViewMut<Unit, track::Insertion>>() {
        Err(GetStorage::TrackingNotEnabled { .. }) => {}
        _ => panic!("expected an error"),
    };

    world.track_insertion::<Unit>();

    assert!(world.borrow::<View<Unit, track::Insertion>>().is_ok());
    assert!(world.borrow::<ViewMut<Unit, track::Insertion>>().is_ok());
}

#[test]
fn runtime_modification_tracking() {
    let mut world = World::new();

    match world.borrow::<View<Unit, track::Modification>>() {
        Err(GetStorage::TrackingNotEnabled { .. }) => {}
        _ => panic!("expected an error"),
    };
    match world.borrow::<ViewMut<Unit, track::Modification>>() {
        Err(GetStorage::TrackingNotEnabled { .. }) => {}
        _ => panic!("expected an error"),
    };

    world.track_modification::<Unit>();

    assert!(world.borrow::<View<Unit, track::Modification>>().is_ok());
    assert!(world.borrow::<ViewMut<Unit, track::Modification>>().is_ok());
}

#[test]
fn runtime_deletion_tracking() {
    let mut world = World::new();

    match world.borrow::<View<Unit, track::Deletion>>() {
        Err(GetStorage::TrackingNotEnabled { .. }) => {}
        _ => panic!("expected an error"),
    };
    match world.borrow::<ViewMut<Unit, track::Deletion>>() {
        Err(GetStorage::TrackingNotEnabled { .. }) => {}
        _ => panic!("expected an error"),
    };

    world.track_deletion::<Unit>();

    assert!(world.borrow::<View<Unit, track::Deletion>>().is_ok());
    assert!(world.borrow::<ViewMut<Unit, track::Deletion>>().is_ok());
}

#[test]
fn runtime_removal_tracking() {
    let mut world = World::new();

    match world.borrow::<View<Unit, track::Removal>>() {
        Err(GetStorage::TrackingNotEnabled { .. }) => {}
        _ => panic!("expected an error"),
    };
    match world.borrow::<ViewMut<Unit, track::Removal>>() {
        Err(GetStorage::TrackingNotEnabled { .. }) => {}
        _ => panic!("expected an error"),
    };

    world.track_removal::<Unit>();

    assert!(world.borrow::<View<Unit, track::Removal>>().is_ok());
    assert!(world.borrow::<ViewMut<Unit, track::Removal>>().is_ok());
}

/// Makes sure we can add runtime tracking to compile time ones
#[test]
fn tracking_inheritance() {
    let mut world = World::new();

    world.track_modification::<UnitInsert>();

    assert!(world.borrow::<View<UnitInsert, track::Insertion>>().is_ok());
    assert!(world
        .borrow::<ViewMut<UnitInsert, track::Insertion>>()
        .is_ok());
    assert!(world
        .borrow::<View<UnitInsert, track::InsertionAndModification>>()
        .is_ok());
    assert!(world
        .borrow::<ViewMut<UnitInsert, track::InsertionAndModification>>()
        .is_ok());
}

#[test]
fn workload_enable_runtime_insertion_tracking() {
    let world = World::new();

    world.add_workload(|| |_: View<Unit, track::Insertion>| {});
    world.run_default_workload().unwrap();

    assert!(world.borrow::<View<Unit, track::Insertion>>().is_ok());
    assert!(world.borrow::<ViewMut<Unit, track::Insertion>>().is_ok());
}

#[test]
fn workload_enable_runtime_modification_tracking() {
    let world = World::new();

    world.add_workload(|| |_: View<Unit, track::Modification>| {});
    world.run_default_workload().unwrap();

    assert!(world.borrow::<View<Unit, track::Modification>>().is_ok());
    assert!(world.borrow::<ViewMut<Unit, track::Modification>>().is_ok());
}

#[test]
fn workload_enable_runtime_deletion_tracking() {
    let world = World::new();

    world.add_workload(|| |_: View<Unit, track::Deletion>| {});
    world.run_default_workload().unwrap();

    assert!(world.borrow::<View<Unit, track::Deletion>>().is_ok());
    assert!(world.borrow::<ViewMut<Unit, track::Deletion>>().is_ok());
}

#[test]
fn workload_enable_runtime_removal_tracking() {
    let world = World::new();

    world.add_workload(|| |_: View<Unit, track::Removal>| {});
    world.run_default_workload().unwrap();

    assert!(world.borrow::<View<Unit, track::Removal>>().is_ok());
    assert!(world.borrow::<ViewMut<Unit, track::Removal>>().is_ok());
}

#[test]
fn clear_all_inserted() {
    let mut world = World::new();

    let eid = world.add_entity(UnitInsert);

    world.run(|unit: View<UnitInsert>| {
        assert!(unit.is_inserted(eid));
    });
    world.run(|unit: View<UnitInsert>| {
        assert!(unit.is_inserted(eid));
    });

    world.clear_all_inserted();

    world.run(|unit: View<UnitInsert>| {
        assert!(!unit.is_inserted(eid));
    });
}

#[test]
fn clear_all_modified() {
    let mut world = World::new();

    let eid = world.add_entity(UnitInsertAndModification);

    world.run(|mut unit: ViewMut<UnitInsertAndModification>| {
        for mut u in (&mut unit).iter() {
            *u = UnitInsertAndModification;
        }
    });

    world.run(|unit: View<UnitInsertAndModification>| {
        assert!(unit.is_modified(eid));
    });
    world.run(|unit: View<UnitInsertAndModification>| {
        assert!(unit.is_modified(eid));
    });

    world.clear_all_modified();

    world.run(|unit: View<UnitInsertAndModification>| {
        assert!(!unit.is_modified(eid));
    });
}
