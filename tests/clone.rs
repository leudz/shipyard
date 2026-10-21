use shipyard::{Component, EntityId, IntoIter, View, World, sparse_set::SparseSet, track};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[derive(Clone)]
struct A(u32);
impl Component for A {
    type Tracking = track::Untracked;
}

#[derive(Clone)]
struct B(u32);
impl Component for B {
    type Tracking = track::Untracked;
}

/// Not cloneable.
struct C;
impl Component for C {
    type Tracking = track::Untracked;
}

/// Returns `(entity, a, b)` for every entity owning A and B, sorted, iterating them together.
fn ab_contents(world: &World) -> Vec<(EntityId, u32, u32)> {
    let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
    let mut contents = Vec::new();
    (&a, &b)
        .iter()
        .with_id()
        .for_each(|(entity, (a, b))| contents.push((entity, a.0, b.0)));
    contents.sort_unstable();

    contents
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

/// Asserts that a world's clone keeps its overlapping groups, a member without a registered
/// clone function included, and iterates the copied components correctly before and after
/// a regroup.
#[test]
fn clone_grouped_world() {
    let mut world = World::builder()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build();
    let first = world.add_entity((A(0), B(10), C));
    let second = world.add_entity((A(1), B(11)));
    world.add_entity((A(2), C));
    world.regroup();
    world.register_clone::<SparseSet<A>>();
    world.register_clone::<SparseSet<B>>();

    let clone = world.clone();

    let expected = vec![(first, 0, 10), (second, 1, 11)];
    assert_eq!(ab_contents(&clone), expected);
    assert!(clone.borrow::<View<C>>().unwrap().is_empty());
    // C isn't cloned, it's still part of the clone's groups.
    clone.regroup_set::<C>().unwrap();
    assert_eq!(ab_contents(&clone), expected);
}
