use shipyard::*;

struct USIZE(usize);
impl Component for USIZE {}

struct U32(u32);
impl Component for U32 {}

#[test]
fn alive() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    world.add_entity((U32(0),));
    let entity = world.add_entity((U32(1),));
    world.add_entity((U32(2),));

    let mut entities = world.borrow::<EntitiesViewMut>().unwrap();

    assert!(entities.spawn(entity));

    assert!(entities.is_alive(entity));
}

#[test]
fn single_dead() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    world.add_entity((U32(0),));
    let entity = world.add_entity((U32(1),));
    world.add_entity((U32(2),));

    world.delete_entity(entity);

    let mut entities = world.borrow::<EntitiesViewMut>().unwrap();

    assert!(entities.spawn(entity));

    assert!(entities.is_alive(entity));
}

#[cfg(feature = "serde1")]
#[test]
fn multiple_dead_first() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let entity0 = world.add_entity((U32(0),));
    let entity1 = world.add_entity((U32(1),));
    let entity2 = world.add_entity((U32(2),));

    world.delete_entity(entity1);
    world.delete_entity(entity0);
    world.delete_entity(entity2);

    let mut entities = world.borrow::<EntitiesViewMut>().unwrap();

    assert!(entities.spawn(entity1));

    assert!(entities.is_alive(entity1));
    assert!(!entities.is_alive(entity0));
    assert!(!entities.is_alive(entity2));

    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(entity0.index(), 1)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(entity2.index(), 1)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(3, 0)
    );
}

#[cfg(feature = "serde1")]
#[test]
fn multiple_dead_middle() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let entity0 = world.add_entity((U32(0),));
    let entity1 = world.add_entity((U32(1),));
    let entity2 = world.add_entity((U32(2),));

    world.delete_entity(entity0);
    world.delete_entity(entity1);
    world.delete_entity(entity2);

    let mut entities = world.borrow::<EntitiesViewMut>().unwrap();

    assert!(entities.spawn(entity1));

    assert!(entities.is_alive(entity1));
    assert!(!entities.is_alive(entity0));
    assert!(!entities.is_alive(entity2));

    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(entity0.index(), 1)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(entity2.index(), 1)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(3, 0)
    );
}

#[cfg(feature = "serde1")]
#[test]
fn multiple_dead_last() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let entity0 = world.add_entity((U32(0),));
    let entity1 = world.add_entity((U32(1),));
    let entity2 = world.add_entity((U32(2),));

    world.delete_entity(entity0);
    world.delete_entity(entity2);
    world.delete_entity(entity1);

    let mut entities = world.borrow::<EntitiesViewMut>().unwrap();

    assert!(entities.spawn(entity1));

    assert!(entities.is_alive(entity1));
    assert!(!entities.is_alive(entity0));
    assert!(!entities.is_alive(entity2));

    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(entity0.index(), 1)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(entity2.index(), 1)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(3, 0)
    );
}

#[cfg(feature = "serde1")]
#[test]
fn new_world_empty() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let entity = EntityId::new_from_index_and_gen(3, 0);

    let mut entities = world.borrow::<EntitiesViewMut>().unwrap();

    assert!(entities.spawn(entity));

    assert!(entities.is_alive(entity));

    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(0, 0)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(1, 0)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(2, 0)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(4, 0)
    );
}

#[cfg(feature = "serde1")]
#[test]
fn new_world() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let entity = EntityId::new_from_index_and_gen(5, 0);

    let entity0 = world.add_entity((U32(0),));
    let entity1 = world.add_entity((U32(1),));
    let entity2 = world.add_entity((U32(2),));

    world.delete_entity(entity0);
    world.delete_entity(entity1);
    world.delete_entity(entity2);

    let mut entities = world.borrow::<EntitiesViewMut>().unwrap();

    assert!(entities.spawn(entity));

    assert!(entities.is_alive(entity));

    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(0, 1)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(1, 1)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(2, 1)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(3, 0)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(4, 0)
    );
    assert_eq!(
        entities.add_entity((), ()),
        EntityId::new_from_index_and_gen(6, 0)
    );
}
