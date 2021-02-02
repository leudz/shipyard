#[cfg(feature = "thread_local")]
mod non_send_sync;

use shipyard::*;

#[test]
fn duplicate_name() {
    let world = World::new();

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
    fn increment(mut i: UniqueViewMut<u32>) {
        *i += 1;
    }

    let world = World::new();

    world.add_unique(0u32).unwrap();

    Workload::builder("Empty")
        .with_system(&increment)
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

    assert_eq!(*world.borrow::<UniqueView<u32>>().unwrap(), 1);
}
