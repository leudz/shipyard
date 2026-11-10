use super::*;
use crate::error;
use crate::scheduler::into_workload_run_if::{IntoRunIf, IntoWorkloadRunIf};

#[test]
fn skip_if_missing_storage() {
    let world = World::new_for_test();

    Workload::new("test")
        .skip_if_storage_empty::<Usize>()
        .with_system(|| panic!())
        .build()
        .unwrap()
        .0
        .run_with_world(&world)
        .unwrap();

    Workload::new("test")
        .skip_if_storage_empty::<Usize>()
        .with_system(|| panic!())
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
}

#[test]
fn system_skip_if_missing_storage() {
    let world = World::new_for_test();

    Workload::new("test")
        .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
        .build()
        .unwrap()
        .0
        .run_with_world(&world)
        .unwrap();

    Workload::new("test")
        .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
}

#[test]
fn skip_if_empty_storage() {
    let mut world = World::new_for_test();

    let eid = world.add_entity((Usize(0),));
    world.remove::<(Usize,)>(eid);

    Workload::new("test")
        .skip_if_storage_empty::<Usize>()
        .with_system(|| -> () { panic!() })
        .build()
        .unwrap()
        .0
        .run_with_world(&world)
        .unwrap();

    Workload::new("test")
        .skip_if_storage_empty::<Usize>()
        .with_system(|| -> () { panic!() })
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
}

#[test]
fn system_skip_if_empty_storage() {
    let mut world = World::new_for_test();

    let eid = world.add_entity((Usize(0),));
    world.remove::<(Usize,)>(eid);

    Workload::new("test")
        .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
        .build()
        .unwrap()
        .0
        .run_with_world(&world)
        .unwrap();

    Workload::new("test")
        .with_system((|| -> () { panic!() }).skip_if_storage_empty::<Usize>())
        .add_to_world(&world)
        .unwrap();

    world.run_default_workload().unwrap();
}

#[test]
fn workload_merge_skip_if() {
    let world = World::new_for_test();

    world.add_unique(Usize(0));

    world.add_workload(|| {
        (
            (|| -> () { panic!() })
                .into_workload()
                .skip_if_missing_unique::<U32>(),
            (|mut u: UniqueViewMut<'_, Usize>| u.0 += 1).into_workload(),
        )
    });

    world.run_default_workload().unwrap();

    assert_eq!(world.borrow::<UniqueView<'_, Usize>>().unwrap().0, 1);
}

#[test]
fn try_system_run_if() {
    fn try_sys() -> Result<(), error::MissingComponent> {
        Err(error::MissingComponent {
            id: crate::EntityId::dead(),
            name: "",
        })
    }

    let (workload, _) = Workload::new("")
        .with_try_system(try_sys.into_workload_try_system().unwrap().run_if(|| true))
        .build()
        .unwrap();

    let world = World::new_for_test();

    assert!(
        workload
            .run_with_world(&world)
            .err()
            .unwrap()
            .custom_error()
            .is_some()
    );
}

/// Returns how converting `condition` into a system condition went, `Ok(())` when it was accepted.
fn system_condition<B, Run: IntoRunIf<B>>(condition: Run) -> Result<(), error::InvalidSystem> {
    IntoRunIf::into_workload_run_if(condition).map(drop)
}

/// Asserts that a system condition borrowing the same storage exclusively and shared
/// is rejected when the two borrows are split across halves.
#[test]
fn condition_across_halves() {
    fn condition(_: ViewMut<'_, Usize>, _: View<'_, U32>, _: View<'_, Usize>) -> bool {
        true
    }

    assert_eq!(
        system_condition(condition),
        Err(error::InvalidSystem::MultipleViews)
    );
}

/// Asserts that a system condition is rejected when its conflicting borrows are its last two.
#[test]
fn condition_last_pair() {
    fn condition(_: View<'_, U32>, _: ViewMut<'_, Usize>, _: View<'_, Usize>) -> bool {
        true
    }

    assert_eq!(
        system_condition(condition),
        Err(error::InvalidSystem::MultipleViews)
    );
}

/// Asserts that a system condition borrowing the same storage exclusively twice is rejected.
#[test]
fn condition_two_mut() {
    fn condition(_: ViewMut<'_, Usize>, _: ViewMut<'_, Usize>) -> bool {
        true
    }

    assert_eq!(
        system_condition(condition),
        Err(error::InvalidSystem::MultipleViewsMut)
    );
}

/// Asserts that a system condition sharing a storage several times is accepted.
#[test]
fn condition_shared() {
    fn condition(_: View<'_, Usize>, _: View<'_, Usize>, _: View<'_, U32>) -> bool {
        true
    }

    assert_eq!(system_condition(condition), Ok(()));
}

/// Asserts that a system condition borrowing the same storage exclusively and shared
/// is rejected when both borrows are in the first half.
#[test]
fn condition_first_half() {
    fn condition(_: ViewMut<'_, Usize>, _: View<'_, Usize>, _: View<'_, U32>) -> bool {
        true
    }

    assert_eq!(
        system_condition(condition),
        Err(error::InvalidSystem::MultipleViews)
    );
}

/// Asserts that a system condition borrowing the same storage exclusively twice
/// is rejected when both borrows are in the first half.
#[test]
fn condition_first_half_mut() {
    fn condition(_: ViewMut<'_, Usize>, _: ViewMut<'_, Usize>, _: View<'_, U32>) -> bool {
        true
    }

    assert_eq!(
        system_condition(condition),
        Err(error::InvalidSystem::MultipleViewsMut)
    );
}

/// Asserts that a system condition borrowing the same storage exclusively and shared
/// is rejected when both borrows are in the second half.
#[test]
fn condition_second_half() {
    fn condition(
        _: View<'_, U16>,
        _: View<'_, U32>,
        _: ViewMut<'_, Usize>,
        _: View<'_, Usize>,
    ) -> bool {
        true
    }

    assert_eq!(
        system_condition(condition),
        Err(error::InvalidSystem::MultipleViews)
    );
}

/// Asserts that a system condition borrowing the same storage exclusively twice
/// is rejected when both borrows are in the second half.
#[test]
fn condition_second_half_mut() {
    fn condition(
        _: View<'_, U16>,
        _: View<'_, U32>,
        _: ViewMut<'_, Usize>,
        _: ViewMut<'_, Usize>,
    ) -> bool {
        true
    }

    assert_eq!(
        system_condition(condition),
        Err(error::InvalidSystem::MultipleViewsMut)
    );
}

/// Asserts that a workload condition borrowing the same storage exclusively and shared
/// is rejected, even with both borrows in the first half.
#[test]
fn workload_condition_first_half() {
    fn condition(_: ViewMut<'_, Usize>, _: View<'_, Usize>, _: View<'_, U32>) -> bool {
        true
    }

    assert!(matches!(
        IntoWorkloadRunIf::into_workload_run_if(condition),
        Err(error::InvalidSystem::MultipleViews)
    ));
}
