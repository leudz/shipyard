#[cfg(feature = "thread_local")]
mod non_send_sync;

struct U64(u64);
impl Component for U64 {
    type Tracking = track::Untracked;
}
impl Unique for U64 {
    type Tracking = track::Untracked;
}

struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}
impl Unique for USIZE {
    type Tracking = track::Untracked;
}

use shipyard::*;

#[test]
fn duplicate_name() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

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
    fn increment(mut i: UniqueViewMut<U64>) {
        i.0 += 1;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.add_unique(U64(0));

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

    assert_eq!(world.borrow::<UniqueView<U64>>().unwrap().0, 1);
}

#[test]
fn are_all_uniques_present_in_world() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.add_unique(U64(0));

    Workload::new("")
        .are_all_uniques_present_in_world(&world)
        .unwrap();

    Workload::new("")
        .with_system(|_: UniqueView<U64>| {})
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
        Err(error::UniquePresence::Unique(type_info).into())
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
        Err(error::UniquePresence::Unique(type_info).into())
    );
}

#[test]
fn run_one_with_world() {
    let world1 = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let world2 = World::new_with_custom_lock::<parking_lot::RawRwLock>();

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
    let world1 = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let world2 = World::new_with_custom_lock::<parking_lot::RawRwLock>();

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
