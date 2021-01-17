use shipyard::*;

fn create_ints(mut _entities: EntitiesViewMut, mut _u32s: ViewMut<u32>) {
    // -- snip --
}

fn delete_ints(mut _u32s: ViewMut<u32>) {
    // -- snip --
}

#[test]
fn test() {
    let world = World::new();

    world.run(create_ints).unwrap();

    Workload::builder("Int cycle")
        .with_system(&create_ints)
        .with_system(&delete_ints)
        .add_to_world(&world)
        .unwrap();
    world.run_workload("Int cycle").unwrap();
    world.run_default().unwrap();
}
