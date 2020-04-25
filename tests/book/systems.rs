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

    world.run(create_ints);

    world
        .add_workload("Int cycle")
        .with_system((|world: &World| world.try_run(create_ints), create_ints))
        .with_system(system!(delete_ints))
        .build();
    world.run_workload("Int cycle");
    world.run_default();
}
