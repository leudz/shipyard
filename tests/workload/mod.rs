#[cfg(feature = "thread_local")]
mod non_send_sync;

struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}
impl Unique for U32 {}

#[allow(unused)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}
impl Unique for USIZE {}

use core::any::type_name;
use shipyard::{borrow::BorrowInfo, scheduler::AsLabel, *};

#[test]
fn duplicate_name() {
    let world = World::new();

    Workload::new("")
        .with_system(|| {})
        .add_to_world(&world)
        .unwrap();
    assert_eq!(
        Workload::new("").add_to_world(&world).err(),
        Some(error::AddWorkload::AlreadyExists)
    );

    world.run_workload("").unwrap();
}

#[test]
fn rename() {
    fn increment(mut i: UniqueViewMut<U32>) {
        i.0 += 1;
    }

    let world = World::new();

    world.add_unique(U32(0));

    Workload::new("Empty")
        .with_system(increment)
        .add_to_world(&world)
        .unwrap();

    world.rename_workload("Empty", "New Empty");

    assert_eq!(
        world
            .run_workload("Empty")
            .err()
            .as_ref()
            .map(std::mem::discriminant),
        Some(std::mem::discriminant(&error::RunWorkload::MissingWorkload))
    );

    world.run_workload("New Empty").unwrap();

    assert_eq!(world.borrow::<UniqueView<U32>>().unwrap().0, 1);
}

#[test]
fn are_all_uniques_present_in_world() {
    let world = World::new();

    world.add_unique(U32(0));

    Workload::new("")
        .are_all_uniques_present_in_world(&world)
        .unwrap();

    Workload::new("")
        .with_system(|_: UniqueView<U32>| {})
        .are_all_uniques_present_in_world(&world)
        .unwrap();

    let type_info = {
        let mut borrow_info = Vec::new();
        UniqueView::<USIZE>::borrow_info(&mut borrow_info);
        borrow_info.remove(0)
    };
    assert_eq!(
        Workload::new("")
            .with_system(|_: UniqueView<USIZE>| {})
            .are_all_uniques_present_in_world(&world),
        Err(error::UniquePresence::Unique(type_info))
    );

    let type_info = {
        let mut borrow_info = Vec::new();
        UniqueViewMut::<USIZE>::borrow_info(&mut borrow_info);
        borrow_info.remove(0)
    };
    assert_eq!(
        Workload::new("")
            .with_system(|_: UniqueViewMut<USIZE>| {})
            .are_all_uniques_present_in_world(&world),
        Err(error::UniquePresence::Unique(type_info))
    );
}

#[test]
fn run_one_with_world() {
    let world1 = World::new();
    let world2 = World::new();

    let builder = Workload::new("").with_system(|| {
        dbg!(1);
    });
    let (workload, _) = builder.build().unwrap();

    workload.run_with_world(&world1).unwrap();
    workload.run_with_world(&world2).unwrap();

    let builder2 = Workload::new("Named").with_system(|| {
        dbg!(1);
    });
    let (workload2, _) = builder2.build().unwrap();

    workload2.run_with_world(&world1).unwrap();
    workload2.run_with_world(&world2).unwrap();
}

#[test]
fn run_with_world() {
    let world1 = World::new();
    let world2 = World::new();

    let builder = Workload::new("")
        .with_system(|| {
            dbg!(1);
        })
        .with_system(|| {
            dbg!(1);
        });
    let (workload, _) = builder.build().unwrap();

    workload.run_with_world(&world1).unwrap();
    workload.run_with_world(&world2).unwrap();

    let builder2 = Workload::new("Named")
        .with_system(|| {
            dbg!(1);
        })
        .with_system(|| {
            dbg!(1);
        });
    let (workload2, _) = builder2.build().unwrap();

    workload2.run_with_world(&world1).unwrap();
    workload2.run_with_world(&world2).unwrap();
}

#[test]
fn enable_tracking() {
    let mut world = World::new();

    world.add_entity(U32(0));

    world.add_workload(|| {
        (|v_u32: View<U32, track::InsertionAndModification>| {
            for _ in v_u32.inserted_or_modified().iter() {}
        })
        .into_workload()
    });

    world.run_default_workload().unwrap();
}

/// System run_if should not run if a workload run_if returns `false`
#[test]
fn check_nested_workloads_run_if() {
    fn sys() {}

    let world = World::new();

    world.add_workload(|| {
        (sys.run_if(|| panic!()).into_workload().run_if(|| false),).into_workload()
    });

    world.run_default_workload().unwrap();
}

#[test]
fn check_run_if_error() {
    fn type_name_of<F: FnOnce() + 'static>(_: F) -> &'static str {
        type_name::<F>()
    }

    fn sys() {}

    let world = World::new();

    world.add_workload(|| (|| {}, sys.run_if(|_: UniqueView<USIZE>| true)).into_workload());

    match world.run_default_workload() {
        Err(error::RunWorkload::Run((label, _))) => {
            assert!(label.dyn_eq(&*type_name_of(sys).as_label()));
        }
        _ => panic!(),
    }
}

#[test]
fn tracking_enabled() {
    fn w() -> Workload {
        (
            |_: View<USIZE, track::All>| {},
            |_: ViewMut<USIZE, track::All>| {},
        )
            .into_workload()
    }

    let world = World::new();

    world.add_workload(w);

    world.run_workload(w).unwrap();
}

#[test]
fn single_system_run_on_same_thread() {
    let world = World::new();
    let main_thread = std::thread::current().id();

    world.add_workload(move || {
        move || {
            assert_eq!(main_thread, std::thread::current().id());
        }
    });

    world.run_default_workload().unwrap();
}

/// Make sure that we don't panic in this scenario:
/// - one system running on main thread
/// - the system has a run_if that evaluates to false
/// - no other system
#[test]
fn skip_first() {
    let world = World::new();

    world.add_workload(|| (|_: AllStoragesViewMut| {}).run_if(|| false));

    world.run_default_workload().unwrap();
}
