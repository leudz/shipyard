use shipyard::{sparse_set::SparseSet, track, Component, View, World};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[test]
fn clone_world() {
    let mut world = World::new();

    let eid = world.add_entity(USIZE(1));

    let world2 = world.clone();

    assert!(world2.borrow::<View<USIZE>>().unwrap().is_empty());

    world.register_clone::<SparseSet<USIZE>>();

    let world3 = world.clone();

    world3.run(|usizes: View<USIZE>| {
        assert_eq!(usizes.len(), 1);

        assert_eq!(usizes[eid], USIZE(1));
    });
}

#[test]
fn clone_entity() {
    let mut world = World::new();

    let eid = world.add_entity(USIZE(1));
    world.add_entity(USIZE(2));

    let mut world2 = world.clone();

    assert!(world2.borrow::<View<USIZE>>().unwrap().is_empty());

    world.register_clone::<SparseSet<USIZE>>();

    world.clone_entity_to(&mut world2, eid);

    world2.run(|usizes: View<USIZE>| {
        assert_eq!(usizes.len(), 1);

        assert_eq!(usizes[eid], USIZE(1));
    });
}
