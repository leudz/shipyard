#[cfg(all(feature = "non_send", feature = "non_sync"))]
mod non_send_sync;

use shipyard::*;

#[test]
fn duplicate_name() {
    let world = World::new();

    Workload::builder("")
        .try_with_system(system!(|| {}))
        .unwrap()
        .add_to_world(&world)
        .unwrap();
    assert_eq!(
        Workload::builder("").add_to_world(&world).err(),
        Some(error::AddWorkload::AlreadyExists)
    );

    world.try_run_workload("").unwrap();
}
