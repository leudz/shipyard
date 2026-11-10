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

/// Returns `true` when a `Usize` was inserted since the condition's last run.
///
/// `Usize` isn't tracked by default, only this condition asks for insertion tracking.
fn inserted(usize: View<'_, Usize, crate::track::Insertion>) -> bool {
    use crate::IntoIter;

    usize.inserted().iter().count() > 0
}

/// Adds one to the `U32` unique, it doesn't borrow `Usize`.
fn body(mut count: UniqueViewMut<'_, U32>) {
    count.0 += 1;
}

/// Returns a world with a `U32` unique and no `Usize` storage.
fn tracking_world() -> World {
    let world = World::new_for_test();
    world.add_unique(U32(0));

    world
}

/// Adds a `Usize`, runs the default workload and returns how many times `body` ran.
fn insert_and_run(world: &mut World) -> u32 {
    world.add_entity(Usize(0));
    world.run_default_workload().unwrap();

    world.borrow::<UniqueView<'_, U32>>().unwrap().0
}

/// Asserts that a system condition is run once its insertion tracking is enabled beforehand.
#[test]
fn tracking_enabled_before() {
    let mut world = tracking_world();
    world.track_insertion::<Usize>();
    Workload::new("")
        .with_system(body.run_if(inserted))
        .add_to_world(&world)
        .unwrap();

    assert_eq!(insert_and_run(&mut world), 1);
}

/// Returns `true` when the world's `Usize` storage tracks insertion.
fn tracks_insertion(world: &World) -> bool {
    world
        .borrow::<View<'_, Usize>>()
        .unwrap()
        .is_tracking_insertion()
}

/// Asserts that registering a system with a `run_if` querying insertion tracking enables it.
#[test]
fn tracking_run_if() {
    let mut world = tracking_world();
    Workload::new("")
        .with_system(body.run_if(inserted))
        .add_to_world(&world)
        .unwrap();

    assert!(tracks_insertion(&world));
    assert_eq!(insert_and_run(&mut world), 1);
}

/// Asserts that registering a system with a `skip_if` querying insertion tracking enables it.
#[test]
fn tracking_skip_if() {
    let mut world = tracking_world();
    Workload::new("")
        .with_system(body.skip_if(inserted))
        .add_to_world(&world)
        .unwrap();

    assert!(tracks_insertion(&world));
    assert_eq!(insert_and_run(&mut world), 0);
}

/// Asserts that a system condition querying insertion tracking enables it
/// when it's chained after another condition.
#[test]
fn tracking_chained() {
    let mut world = tracking_world();
    Workload::new("")
        .with_system(body.run_if(|| true).run_if(inserted))
        .add_to_world(&world)
        .unwrap();

    assert!(tracks_insertion(&world));
    assert_eq!(insert_and_run(&mut world), 1);
}

/// Asserts that registering a workload with a condition querying insertion tracking enables it.
#[test]
fn tracking_workload() {
    let mut world = tracking_world();
    Workload::new("")
        .with_system(body)
        .run_if(inserted)
        .add_to_world(&world)
        .unwrap();

    assert!(tracks_insertion(&world));
    assert_eq!(insert_and_run(&mut world), 1);
}

/// Asserts that a workload condition querying insertion tracking enables it
/// when it's chained after another condition.
#[test]
fn tracking_workload_chained() {
    let mut world = tracking_world();
    Workload::new("")
        .with_system(body)
        .run_if(|| true)
        .run_if(inserted)
        .add_to_world(&world)
        .unwrap();

    assert!(tracks_insertion(&world));
    assert_eq!(insert_and_run(&mut world), 1);
}

/// Asserts that registering a workload whose nested workload has a condition
/// querying insertion tracking enables it.
#[test]
fn tracking_nested() {
    let mut world = tracking_world();
    let child = Workload::new("child").with_system(body).run_if(inserted);
    Workload::new("")
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();

    assert!(tracks_insertion(&world));
    assert_eq!(insert_and_run(&mut world), 1);
}

/// Asserts that a nested workload's condition querying insertion tracking enables it
/// when it's combined with a system condition.
#[test]
fn tracking_nested_combined() {
    let mut world = tracking_world();
    let child = Workload::new("child")
        .with_system(body.run_if(|| true))
        .run_if(inserted);
    Workload::new("")
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();

    assert!(tracks_insertion(&world));
    assert_eq!(insert_and_run(&mut world), 1);
}

/// Asserts that `apply_tracking` on a built workload enables the tracking its system condition queries.
#[test]
fn tracking_built() {
    let mut world = tracking_world();
    let (workload, _) = Workload::new("")
        .with_system(body.run_if(inserted))
        .build()
        .unwrap();
    workload.apply_tracking(&world).unwrap();

    assert!(tracks_insertion(&world));
    world.add_entity(Usize(0));
    workload.run_with_world(&world).unwrap();
    assert_eq!(world.borrow::<UniqueView<'_, U32>>().unwrap().0, 1);
}

/// Asserts that `apply_tracking` on a built workload enables the tracking its own condition queries.
#[test]
fn tracking_built_workload() {
    let mut world = tracking_world();
    let (workload, _) = Workload::new("")
        .with_system(body)
        .run_if(inserted)
        .build()
        .unwrap();
    workload.apply_tracking(&world).unwrap();

    assert!(tracks_insertion(&world));
    world.add_entity(Usize(0));
    workload.run_with_world(&world).unwrap();
    assert_eq!(world.borrow::<UniqueView<'_, U32>>().unwrap().0, 1);
}

/// Adds one to the `U16` unique.
fn count_u16(mut count: UniqueViewMut<'_, U16>) {
    count.0 += 1;
}

/// Returns a world tracking `Usize` insertion, with `U32` and `U16` uniques at 0.
fn cursor_world() -> World {
    let mut world = World::new_for_test();
    world.track_insertion::<Usize>();
    world.add_unique(U32(0));
    world.add_unique(U16(0));

    world
}

/// Runs the default workload three times: after an insertion, without one, after another one.
///
/// Returns how many times `body` and `count_u16` ran so far, after each run.
fn cursor_runs(world: &mut World) -> Vec<(u32, u16)> {
    let mut counts = Vec::new();

    for insert in [true, false, true] {
        if insert {
            world.add_entity(Usize(0));
        }
        world.run_default_workload().unwrap();

        let first = world.borrow::<UniqueView<'_, U32>>().unwrap().0;
        let second = world.borrow::<UniqueView<'_, U16>>().unwrap().0;
        counts.push((first, second));
    }

    counts
}

/// Asserts that a workload's condition on insertion tracking lets all its systems run after each insertion,
/// and none without one.
#[test]
fn cursor_top_level() {
    let mut world = cursor_world();
    Workload::new("")
        .with_system(body)
        .with_system(count_u16)
        .run_if(inserted)
        .add_to_world(&world)
        .unwrap();

    assert_eq!(cursor_runs(&mut world), [(1, 1), (1, 1), (2, 2)]);
}

/// Asserts that a nested workload's constant condition lets all its systems run every time.
#[test]
fn cursor_constant() {
    let mut world = cursor_world();
    let child = Workload::new("child")
        .with_system(body)
        .with_system(count_u16)
        .run_if(|| true);
    Workload::new("")
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();

    assert_eq!(cursor_runs(&mut world), [(1, 1), (2, 2), (3, 3)]);
}

/// Does nothing with the `U32` unique, a parent's unrelated work.
fn unrelated(_: View<'_, U32>) {}

/// Asserts that a nested workload's condition on insertion tracking lets all its systems run
/// after each insertion, like the same condition on a top-level workload.
#[test]
fn cursor_nested() {
    let mut world = cursor_world();
    let child = Workload::new("child")
        .with_system(body)
        .with_system(count_u16)
        .run_if(inserted);
    Workload::new("")
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();

    assert_eq!(cursor_runs(&mut world), [(1, 1), (1, 1), (2, 2)]);
}

/// Asserts that a nested workload's condition on insertion tracking lets both its systems run
/// after each insertion, with a barrier between them.
#[test]
fn cursor_nested_barrier() {
    let mut world = cursor_world();
    let child = Workload::new("child")
        .with_system(body)
        .with_barrier()
        .with_system(count_u16)
        .run_if(inserted);
    Workload::new("")
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();

    assert_eq!(cursor_runs(&mut world), [(1, 1), (1, 1), (2, 2)]);
}

/// Asserts that a nested workload's condition on insertion tracking lets all its systems run
/// after each insertion, next to unrelated parent work.
#[test]
fn cursor_nested_unrelated() {
    let mut world = cursor_world();
    let child = Workload::new("child")
        .with_system(body)
        .with_system(count_u16)
        .run_if(inserted);
    Workload::new("")
        .with_system(unrelated)
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();

    assert_eq!(cursor_runs(&mut world), [(1, 1), (1, 1), (2, 2)]);
}

/// Asserts that a nested workload's condition on insertion tracking lets all its systems run
/// after each insertion, and none without one, when each system has its own condition.
#[test]
fn cursor_nested_combined() {
    let mut world = cursor_world();
    let child = Workload::new("child")
        .with_system(body.run_if(|| true))
        .with_system(count_u16.run_if(|| true))
        .run_if(inserted);
    Workload::new("")
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();

    assert_eq!(cursor_runs(&mut world), [(1, 1), (1, 1), (2, 2)]);
}

/// Asserts that a nested workload's condition on insertion tracking, chained after another condition,
/// lets all its systems run after each insertion.
#[test]
fn cursor_nested_chained() {
    let mut world = cursor_world();
    let child = Workload::new("child")
        .with_system(body)
        .with_system(count_u16)
        .run_if(|| true)
        .run_if(inserted);
    Workload::new("")
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();

    assert_eq!(cursor_runs(&mut world), [(1, 1), (1, 1), (2, 2)]);
}

/// Asserts that a nested workload's `skip_if` on insertion tracking skips all its systems
/// after each insertion, and runs them all without one.
#[test]
fn cursor_nested_skip_if() {
    let mut world = cursor_world();
    let child = Workload::new("child")
        .with_system(body)
        .with_system(count_u16)
        .skip_if(inserted);
    Workload::new("")
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();

    assert_eq!(cursor_runs(&mut world), [(0, 0), (1, 1), (1, 1)]);
}

/// Asserts that a workload's condition on insertion tracking, nested two levels deep,
/// lets all its systems run after each insertion.
#[test]
fn cursor_nested_deep() {
    let mut world = cursor_world();
    let grandchild = Workload::new("grandchild")
        .with_system(body)
        .with_system(count_u16)
        .run_if(inserted);
    let child = Workload::new("child").with_workload(grandchild);
    Workload::new("")
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();

    assert_eq!(cursor_runs(&mut world), [(1, 1), (1, 1), (2, 2)]);
}

/// Asserts that a system condition skipped by its nested workload's false condition
/// sees the insertion made while it was skipped, once the workload's condition is true.
#[test]
fn cursor_skipped_window() {
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicBool, Ordering};

    let mut world = cursor_world();
    let open = Arc::new(AtomicBool::new(false));
    let gate = {
        let open = open.clone();
        move || open.load(Ordering::Relaxed)
    };
    let child = Workload::new("child")
        .with_system(body.run_if(inserted))
        .run_if(gate);
    Workload::new("")
        .with_workload(child)
        .add_to_world(&world)
        .unwrap();
    let mut runs = Vec::new();

    world.add_entity(Usize(0));
    world.run_default_workload().unwrap();
    runs.push(world.borrow::<UniqueView<'_, U32>>().unwrap().0);
    open.store(true, Ordering::Relaxed);
    world.run_default_workload().unwrap();
    runs.push(world.borrow::<UniqueView<'_, U32>>().unwrap().0);

    assert_eq!(runs, [0, 1]);
}
