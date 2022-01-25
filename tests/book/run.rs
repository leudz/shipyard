use super::USIZE;
use shipyard::*;

#[test]
fn all() {
    let world = World::new();

    world.run(|mut _all_storages: AllStoragesViewMut| {
        // -- snip --
    });
}

#[test]
fn multiple() {
    let world = World::new();

    world.run(|all_storages: AllStoragesViewMut| {
        // do something with all_storages

        all_storages.run(|_usizes: View<USIZE>| {
            // -- snip --
        });
    });
}
