use shipyard::{
    borrow::NonSendSync, iter::IntoIter, track, Component, EntitiesViewMut, View, ViewMut, World,
};
use std::cell::RefCell;
use std::rc::Rc;

#[allow(unused)]
struct UnitInsertAndModification(Rc<RefCell<String>>);
impl Component for UnitInsertAndModification {
    type Tracking = track::InsertionAndModification;
}

impl UnitInsertAndModification {
    fn new() -> UnitInsertAndModification {
        UnitInsertAndModification(Rc::new(RefCell::new(String::new())))
    }
}

/// Makes sure `clear_all_inserted`, `clear_all_modified` and `clear_all_inserted_and_modified`
/// can be called.
///
/// They are particular, implemented on `NonSend`, `NonSync` and `NonSendSync`.
#[test]
fn compile_check() {
    let world = World::new();

    world.run(
        |mut entities: EntitiesViewMut,
         mut unit: NonSendSync<ViewMut<UnitInsertAndModification>>| {
            entities.add_entity(&mut *unit, UnitInsertAndModification::new());
        },
    );

    world.run(|unit: NonSendSync<ViewMut<UnitInsertAndModification>>| {
        unit.clear_all_inserted();
    });

    world.run(|unit: NonSendSync<ViewMut<UnitInsertAndModification>>| {
        unit.clear_all_modified();
    });

    world.run(|unit: NonSendSync<ViewMut<UnitInsertAndModification>>| {
        unit.clear_all_inserted_and_modified();
    });
}

#[test]
fn clear_all_inserted_thread_local() {
    let mut world = World::new();

    let eid = world.run(
        |mut entities: EntitiesViewMut,
         mut unit: NonSendSync<ViewMut<UnitInsertAndModification>>| {
            entities.add_entity(&mut *unit, UnitInsertAndModification::new())
        },
    );

    world.run(|unit: NonSendSync<View<UnitInsertAndModification>>| {
        assert!(unit.is_inserted(eid));
    });
    world.run(|unit: NonSendSync<View<UnitInsertAndModification>>| {
        assert!(unit.is_inserted(eid));
    });

    world.clear_all_inserted();

    world.run(|unit: NonSendSync<View<UnitInsertAndModification>>| {
        assert!(!unit.is_inserted(eid));
    });
}

#[test]
fn clear_all_modified_thread_local() {
    let mut world = World::new();

    let eid = world.run(
        |mut entities: EntitiesViewMut,
         mut unit: NonSendSync<ViewMut<UnitInsertAndModification>>| {
            entities.add_entity(&mut *unit, UnitInsertAndModification::new())
        },
    );

    world.run(
        |mut unit: NonSendSync<ViewMut<UnitInsertAndModification>>| {
            for mut u in (&mut *unit).iter() {
                *u = UnitInsertAndModification::new();
            }
        },
    );

    world.run(|unit: NonSendSync<View<UnitInsertAndModification>>| {
        assert!(unit.is_modified(eid));
    });
    world.run(|unit: NonSendSync<View<UnitInsertAndModification>>| {
        assert!(unit.is_modified(eid));
    });

    world.clear_all_modified();

    world.run(|unit: NonSendSync<View<UnitInsertAndModification>>| {
        assert!(!unit.is_modified(eid));
    });
}
