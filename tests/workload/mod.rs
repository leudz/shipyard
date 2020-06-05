#[cfg(all(feature = "non_send", feature = "non_sync"))]
mod non_send_sync;

use shipyard::*;

#[test]
fn duplicate_name() {
    let world = World::new();

    world
        .try_add_workload("")
        .unwrap()
        .try_with_system(system!(|| {}))
        .unwrap()
        .build();
    assert_eq!(
        world.try_add_workload("").err(),
        Some(error::AddWorkload::AlreadyExists)
    );

    world.try_run_workload("").unwrap();
}
