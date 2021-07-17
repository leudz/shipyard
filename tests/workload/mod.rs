#[cfg(feature = "thread_local")]
mod non_send_sync;

struct U32(u32);
impl Component for U32 {
    type Tracking = track::Nothing;
}

use shipyard::*;

#[test]
fn duplicate_name() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    Workload::builder("")
        .with_system(|| {})
        .add_to_world(&world)
        .unwrap();
    assert_eq!(
        Workload::builder("").add_to_world(&world).err(),
        Some(error::AddWorkload::AlreadyExists)
    );

    world.run_workload("").unwrap();
}

#[test]
fn rename() {
    fn increment(mut i: UniqueViewMut<U32>) {
        i.0 += 1;
    }

    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.add_unique(U32(0)).unwrap();

    Workload::builder("Empty")
        .with_system(increment)
        .add_to_world(&world)
        .unwrap();

    world.rename_workload("Empty", "New Empty").unwrap();

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
