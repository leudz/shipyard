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
