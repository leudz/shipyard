#[cfg(feature = "thread_local")]
mod thread_local;

use core::marker::PhantomData;
use shipyard::advanced::tracking::{
    InsertionOrModificationTracking, InsertionTracking, ModificationTracking, Tracking,
};
use shipyard::{Component, IntoIter, View, ViewMut, World, error::GetStorage, track};

struct Probe<Track>(u32, PhantomData<Track>);
impl<Track: Tracking> Component for Probe<Track> {
    type Tracking = Track;
}

fn probe<Track: Tracking>(value: u32) -> Probe<Track> {
    Probe(value, PhantomData)
}

/// Asserts that every built-in mode tracking insertion or modification also has the combined capability.
#[test]
fn capability_matrix() {
    fn combined<Track: InsertionOrModificationTracking>() {}
    fn insertion<Track: InsertionTracking>() {
        combined::<Track>();
    }
    fn modification<Track: ModificationTracking>() {
        combined::<Track>();
    }

    insertion::<track::Insertion>();
    insertion::<track::InsertionAndDeletion>();
    insertion::<track::InsertionAndRemoval>();
    insertion::<track::InsertionAndDeletionAndRemoval>();
    modification::<track::Modification>();
    modification::<track::ModificationAndDeletion>();
    modification::<track::ModificationAndRemoval>();
    modification::<track::ModificationAndDeletionAndRemoval>();
    insertion::<track::InsertionAndModification>();
    insertion::<track::InsertionAndModificationAndDeletion>();
    insertion::<track::InsertionAndModificationAndRemoval>();
    insertion::<track::All>();
    modification::<track::InsertionAndModification>();
    modification::<track::InsertionAndModificationAndDeletion>();
    modification::<track::InsertionAndModificationAndRemoval>();
    modification::<track::All>();
}

/// Asserts that insertion-only combined queries and clearing ignore modification timestamps.
#[test]
fn combined_insertion() {
    let mut world = World::new();
    let old = world.add_entity((probe::<track::Insertion>(1),));
    world
        .borrow::<ViewMut<Probe<track::Insertion>>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    let recent = world.add_entity((probe::<track::Insertion>(2),));

    let view = world.borrow::<View<Probe<track::Insertion>>>().unwrap();
    assert!(!view.is_inserted_or_modified(old));
    assert!(view.is_inserted_or_modified(recent));
    assert_eq!(
        view.inserted_or_modified()
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(
        (!view.inserted_or_modified())
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        vec![1]
    );
    drop(view);

    let view = world.borrow::<ViewMut<Probe<track::Insertion>>>().unwrap();
    assert!(view.is_inserted_or_modified(recent));
    view.clear_all_inserted_and_modified();
    let view = world.borrow::<View<Probe<track::Insertion>>>().unwrap();
    assert!(!view.is_inserted_or_modified(recent));
    drop(view);
    let later = world.add_entity((probe::<track::Insertion>(3),));
    let view = world.borrow::<View<Probe<track::Insertion>>>().unwrap();
    assert!(view.is_inserted_or_modified(later));
    assert!(!view.is_inserted_or_modified(shipyard::EntityId::dead()));
}

/// Asserts that modification-only combined queries and clearing ignore insertion timestamps.
#[test]
fn combined_modification() {
    let mut world = World::new();
    let changed = world.add_entity((probe::<track::Modification>(1),));
    let untouched = world.add_entity((probe::<track::Modification>(2),));
    {
        let mut view = world
            .borrow::<ViewMut<Probe<track::Modification>>>()
            .unwrap();
        (&mut view).iter().next().unwrap().0 += 10;
    }

    let view = world.borrow::<View<Probe<track::Modification>>>().unwrap();
    assert!(view.is_inserted_or_modified(changed));
    assert!(!view.is_inserted_or_modified(untouched));
    assert_eq!(
        view.inserted_or_modified()
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        vec![11]
    );
    assert_eq!(
        (!view.inserted_or_modified())
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        vec![2]
    );
    drop(view);

    world
        .borrow::<ViewMut<Probe<track::Modification>>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    let view = world.borrow::<View<Probe<track::Modification>>>().unwrap();
    assert!(!view.is_inserted_or_modified(changed));
    drop(view);

    let mut view = world
        .borrow::<ViewMut<Probe<track::Modification>>>()
        .unwrap();
    (&mut view).iter().nth(1).unwrap().0 += 10;
    assert!(!view.is_inserted_or_modified(changed));
    assert!(view.is_inserted_or_modified(untouched));
}

/// Asserts that combined tracking yields the union once, negates it, and records mutable iteration.
#[test]
fn combined_both() {
    let mut world = World::new();
    let neither = world.add_entity((probe::<track::InsertionAndModification>(0),));
    let modified = world.add_entity((probe::<track::InsertionAndModification>(1),));
    world
        .borrow::<ViewMut<Probe<track::InsertionAndModification>>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    let inserted = world.add_entity((probe::<track::InsertionAndModification>(3),));
    let both = world.add_entity((probe::<track::InsertionAndModification>(4),));

    {
        let mut view = world
            .borrow::<ViewMut<Probe<track::InsertionAndModification>>>()
            .unwrap();
        (&mut view).iter().nth(1).unwrap().0 += 10;
        (&mut view).iter().nth(3).unwrap().0 += 10;
    }

    let view = world
        .borrow::<View<Probe<track::InsertionAndModification>>>()
        .unwrap();
    assert!(!view.is_inserted_or_modified(neither));
    assert!(view.is_inserted_or_modified(modified));
    assert!(view.is_inserted_or_modified(inserted));
    assert!(view.is_inserted_or_modified(both));
    assert_eq!(
        view.inserted_or_modified()
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        vec![11, 3, 14]
    );
    assert_eq!(
        (!view.inserted_or_modified())
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        vec![0]
    );
    drop(view);

    let mut view = world
        .borrow::<ViewMut<Probe<track::InsertionAndModification>>>()
        .unwrap();
    assert_eq!(
        view.inserted_or_modified()
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        vec![11, 3, 14]
    );
    for mut value in view.inserted_or_modified_mut().iter() {
        value.0 += 100;
    }
    assert_eq!(
        view.inserted_or_modified()
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        vec![111, 103, 114]
    );
    view.clear_all_inserted_and_modified();

    let view = world
        .borrow::<View<Probe<track::InsertionAndModification>>>()
        .unwrap();
    assert_eq!(view.inserted_or_modified().iter().count(), 0);
    drop(view);
    let later = world.add_entity((probe::<track::InsertionAndModification>(5),));
    let view = world
        .borrow::<View<Probe<track::InsertionAndModification>>>()
        .unwrap();
    assert!(view.is_inserted_or_modified(later));
}

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
    assert!(
        world
            .borrow::<ViewMut<UnitInsert, track::Insertion>>()
            .is_ok()
    );
    assert!(
        world
            .borrow::<View<UnitInsert, track::InsertionAndModification>>()
            .is_ok()
    );
    assert!(
        world
            .borrow::<ViewMut<UnitInsert, track::InsertionAndModification>>()
            .is_ok()
    );
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
