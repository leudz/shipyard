use shipyard::{
    borrow::NonSendSync, iter::IntoIter, track, Component, EntitiesViewMut, View, ViewMut, World,
};
use std::cell::RefCell;
use std::rc::Rc;

struct InsertOnly;
impl Component for InsertOnly {
    type Tracking = track::Insertion;
}

struct ModifyOnly(u32);
impl Component for ModifyOnly {
    type Tracking = track::Modification;
}

#[allow(unused)]
struct LocalInsert(Rc<RefCell<u32>>);
impl Component for LocalInsert {
    type Tracking = track::Insertion;
}

/// Asserts that thread-local views clear and check combined tracking with either individual capability.
#[test]
fn combined_individual_wrappers() {
    use shipyard::borrow::{NonSend, NonSync};

    let world = World::new();
    let inserted = world.run(
        |mut entities: EntitiesViewMut, mut view: NonSend<ViewMut<InsertOnly>>| {
            entities.add_entity(&mut *view, InsertOnly)
        },
    );
    let modified = world.run(
        |mut entities: EntitiesViewMut, mut view: NonSync<ViewMut<ModifyOnly>>| {
            entities.add_entity(&mut *view, ModifyOnly(1))
        },
    );

    world.run(|view: NonSend<View<InsertOnly>>| {
        assert!(view.is_inserted_or_modified(inserted));
    });
    world.run(|view: NonSend<ViewMut<InsertOnly>>| {
        view.clear_all_inserted_and_modified();
    });
    world.run(|view: NonSend<View<InsertOnly>>| {
        assert!(!view.is_inserted_or_modified(inserted));
    });

    world.run(|mut view: NonSync<ViewMut<ModifyOnly>>| {
        (&mut *view).iter().next().unwrap().0 += 1;
        assert!(view.is_inserted_or_modified(modified));
    });
    world.run(|view: NonSync<ViewMut<ModifyOnly>>| {
        view.clear_all_inserted_and_modified();
    });
    world.run(|view: NonSync<View<ModifyOnly>>| {
        assert!(!view.is_inserted_or_modified(modified));
    });

    let local = world.run(
        |mut entities: EntitiesViewMut, mut view: NonSendSync<ViewMut<LocalInsert>>| {
            entities.add_entity(&mut *view, LocalInsert(Rc::new(RefCell::new(1))))
        },
    );
    world.run(|view: NonSendSync<View<LocalInsert>>| {
        assert!(view.is_inserted_or_modified(local));
    });
    world.run(|view: NonSendSync<ViewMut<LocalInsert>>| {
        view.clear_all_inserted_and_modified();
    });
    world.run(|view: NonSendSync<View<LocalInsert>>| {
        assert!(!view.is_inserted_or_modified(local));
    });
}

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
