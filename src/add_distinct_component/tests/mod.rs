use super::*;
use crate::{Component, EntitiesViewMut, Get, ViewMut, World, track};

#[derive(PartialEq, Debug)]
struct USIZE(usize);

impl Component for USIZE {
    type Tracking = track::InsertionAndModification;
}

/// Make sure that:
/// - `add_distinct_component_unchecked` inserts the component when no component is present.
/// - it correctly reports the success.
/// - the component is flagged as inserted.
/// - the component is not flagged as modified.
#[test]
fn add_no_component() {
    let mut world = World::new_for_test();

    let eid = world.add_entity(());

    let mut usizes = world.borrow::<ViewMut<'_, USIZE>>().unwrap();

    let was_inserted = usizes.add_distinct_component_unchecked(eid, USIZE(0));

    assert!(was_inserted);
    assert_eq!(usizes.get(eid).unwrap(), &USIZE(0));
    assert!(usizes.is_inserted(eid));
    assert!(!usizes.is_modified(eid));
}

/// Make sure that:
/// - `add_distinct_component_unchecked` inserts the component when a distinct component is present.
/// - it correctly reports the success.
/// - the component is not flagged as inserted.
/// - the component is flagged as modified.
#[test]
fn add_distinct_component() {
    let mut world = World::new_for_test();

    let eid = world.add_entity(USIZE(1));

    world.run(|usizes: ViewMut<'_, USIZE>| {
        usizes.clear_all_inserted();
    });

    let mut usizes = world.borrow::<ViewMut<'_, USIZE>>().unwrap();

    let was_inserted = usizes.add_distinct_component_unchecked(eid, USIZE(0));

    assert!(was_inserted);
    assert_eq!(usizes.get(eid).unwrap(), &USIZE(0));
    assert!(!usizes.is_inserted(eid));
    assert!(usizes.is_modified(eid));
}

/// Make sure that:
/// - `add_distinct_component_unchecked` does not insert the component when an equal component is present.
/// - it correctly reports the failure.
/// - the component is not flagged as inserted.
/// - the component is not flagged as modified.
#[test]
fn add_identical_component() {
    let mut world = World::new_for_test();

    let eid = world.add_entity(USIZE(0));

    world.run(|usizes: ViewMut<'_, USIZE>| {
        usizes.clear_all_inserted();
    });

    let mut usizes = world.borrow::<ViewMut<'_, USIZE>>().unwrap();

    let was_inserted = usizes.add_distinct_component_unchecked(eid, USIZE(0));

    assert!(!was_inserted);
    assert!(!usizes.is_inserted(eid));
    assert!(!usizes.is_modified(eid));
}

/// Make sure that:
/// - `add_distinct_component_unchecked` does not insert the component when a component from an entity with a larger generation is present.
/// - it correctly reports the failure.
/// - the component is not flagged as inserted.
/// - the component is not flagged as modified.
#[test]
fn add_smaller_gen_component() {
    let mut world = World::new_for_test();

    let eid1 = world.add_entity(());
    world.delete_entity(eid1);
    let eid2 = world.add_entity(USIZE(1));

    assert_eq!(eid1.index(), eid2.index());

    let mut usizes = world.borrow::<ViewMut<'_, USIZE>>().unwrap();

    let was_inserted = usizes.add_distinct_component_unchecked(eid1, USIZE(0));

    assert!(usizes.get(eid1).is_err());
    assert_eq!(usizes.get(eid2).unwrap(), &USIZE(1));

    assert!(!was_inserted);
    assert!(!usizes.is_inserted(eid1));
    assert!(!usizes.is_modified(eid1));
}

/// Make sure that:
/// - `add_distinct_component_unchecked` inserts the component when a component from an entity with a smaller generation is present.
/// - it correctly reports the success.
/// - the component is flagged as inserted.
/// - the component is not flagged as modified.
#[test]
fn add_larger_gen_component() {
    let mut world = World::new_for_test();

    let eid1 = world.add_entity(USIZE(1));

    world.run(|mut entities: EntitiesViewMut<'_>| {
        entities.delete_unchecked(eid1);
    });

    let eid2 = world.add_entity(());

    assert_eq!(eid1.index(), eid2.index());

    let mut usizes = world.borrow::<ViewMut<'_, USIZE>>().unwrap();

    let was_inserted = usizes.add_distinct_component_unchecked(eid2, USIZE(0));

    assert!(usizes.get(eid1).is_err());
    assert_eq!(usizes.get(eid2).unwrap(), &USIZE(0));

    assert!(was_inserted);
    assert!(usizes.is_inserted(eid2));
    assert!(!usizes.is_modified(eid2));
}
