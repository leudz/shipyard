use shipyard::{
    AllStoragesView, AllStoragesViewMut, Component, EntityId, IntoIter, Unique, UniqueStorage,
    UniqueViewMut, View, World, sparse_set::SparseSet, track,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

#[derive(Clone)]
struct Counter;
impl Unique for Counter {}

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

/// Asserts that a world's clone has the same alive entities and reuses deleted ids like the
/// original does.
#[test]
fn clone_entities() {
    let mut world = World::new();
    let deleted = world.add_entity(());
    let alive = world.add_entity(());
    world.delete_entity(deleted);

    let mut clone = world.clone();

    assert!(!clone.is_entity_alive(deleted));
    assert!(clone.is_entity_alive(alive));
    assert_eq!(clone.add_entity(()), world.add_entity(()));
}

/// Asserts that a world's clone doesn't get the original's deletion callback.
#[test]
fn clone_without_on_deletion() {
    let mut world = World::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    world.on_deletion(move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
    });
    let entity = world.add_entity(());

    let mut clone = world.clone();
    clone.delete_entity(entity);
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    world.delete_entity(entity);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

/// Asserts that cloning storages into an existing world replaces its entities, deletion callback
/// included.
#[test]
fn clone_storages_replace_entities() {
    let mut source = World::new();
    let entity = source.add_entity(());
    let mut destination = World::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    destination.on_deletion(move |_| {
        counter.fetch_add(1, Ordering::Relaxed);
    });

    let source_storages = source.borrow::<AllStoragesView>().unwrap();
    let mut destination_storages = destination.borrow::<AllStoragesViewMut>().unwrap();
    source_storages.clone_storages_to(&mut destination_storages);
    drop((source_storages, destination_storages));

    assert!(destination.is_entity_alive(entity));
    destination.delete_entity(entity);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

/// Asserts that cloning an entity to another world works while a unique is borrowed exclusively,
/// uniques don't hold components.
#[test]
fn clone_entity_borrowed_unique() {
    let mut world = World::new();
    world.add_unique(Counter);
    world.register_clone::<SparseSet<USIZE>>();
    let entity = world.add_entity(USIZE(1));
    let other = World::new();

    let all_storages = world.borrow::<AllStoragesView>().unwrap();
    let unique = all_storages.borrow::<UniqueViewMut<Counter>>().unwrap();
    let mut other_storages = other.borrow::<AllStoragesViewMut>().unwrap();
    all_storages.clone_entity_to(&mut other_storages, entity);
    drop(other_storages);
    drop(unique);
    drop(all_storages);

    assert_eq!(other.borrow::<View<USIZE>>().unwrap()[entity], USIZE(1));
}

/// Asserts that cloning components to another world's entity works while a unique is borrowed
/// exclusively, uniques don't hold components.
#[test]
fn clone_components_borrowed_unique() {
    let mut world = World::new();
    world.add_unique(Counter);
    world.register_clone::<SparseSet<USIZE>>();
    let from = world.add_entity(USIZE(1));
    let mut other = World::new();
    let to = other.add_entity(());

    let all_storages = world.borrow::<AllStoragesView>().unwrap();
    let unique = all_storages.borrow::<UniqueViewMut<Counter>>().unwrap();
    let mut other_storages = other.borrow::<AllStoragesViewMut>().unwrap();
    all_storages.clone_components_to(&mut other_storages, from, to);
    drop(other_storages);
    drop(unique);
    drop(all_storages);

    assert_eq!(other.borrow::<View<USIZE>>().unwrap()[to], USIZE(1));
}

/// Asserts that cloning all storages still fails while a unique is borrowed exclusively, the
/// uniques are cloned too.
#[test]
#[should_panic]
fn clone_storages_borrowed_unique() {
    let mut world = World::new();
    world.add_unique(Counter);
    world.register_clone::<UniqueStorage<Counter>>();
    let other = World::new();

    let all_storages = world.borrow::<AllStoragesView>().unwrap();
    let _unique = all_storages.borrow::<UniqueViewMut<Counter>>().unwrap();
    let mut other_storages = other.borrow::<AllStoragesViewMut>().unwrap();
    all_storages.clone_storages_to(&mut other_storages);
}
