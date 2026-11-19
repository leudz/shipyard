mod miri;

use super::*;

#[test]
fn entities() {
    let mut entities = Entities::new();

    let key00 = entities.generate();
    let key10 = entities.generate();

    assert_eq!(key00.index(), 0);
    assert_eq!(key00.generation(), 0);
    assert_eq!(key10.index(), 1);
    assert_eq!(key10.generation(), 0);

    assert!(entities.delete_unchecked(key00));
    assert!(!entities.delete_unchecked(key00));
    let key01 = entities.generate();

    assert_eq!(key01.index(), 0);
    assert_eq!(key01.generation(), 1);

    assert!(entities.delete_unchecked(key10));
    assert!(entities.delete_unchecked(key01));
    let key11 = entities.generate();
    let key02 = entities.generate();

    assert_eq!(key11.index(), 1);
    assert_eq!(key11.generation(), 1);
    assert_eq!(key02.index(), 0);
    assert_eq!(key02.generation(), 2);

    let last_key = EntityId::new_from_index_and_gen(0, EntityId::max_gen());
    entities.data[0] = last_key;
    assert!(entities.delete_unchecked(last_key));
    assert_eq!(entities.list, None);
    let dead = entities.generate();
    assert_eq!(dead.index(), 2);
    assert_eq!(dead.generation(), 0);
}

#[test]
fn iterator() {
    let mut entities = Entities::new();

    entities.add_entity((), ());
    entities.add_entity((), ());
    entities.add_entity((), ());

    let mut iter = entities.iter();

    let id0 = iter.next().unwrap();
    assert_eq!(id0.index(), 0);
    assert_eq!(id0.generation(), 0);

    let id1 = iter.next().unwrap();
    assert_eq!(id1.index(), 1);
    assert_eq!(id1.generation(), 0);

    let id2 = iter.next().unwrap();
    assert_eq!(id2.index(), 2);
    assert_eq!(id2.generation(), 0);

    assert!(iter.next().is_none());

    entities.delete_unchecked(id0);
    entities.delete_unchecked(id1);
    entities.add_entity((), ());

    let mut iter = entities.iter();

    let id = iter.next().unwrap();
    assert_eq!(id.index(), 0);
    assert_eq!(id.generation(), 1);

    let id = iter.next().unwrap();
    assert_eq!(id.index(), 2);
    assert_eq!(id.generation(), 0);

    assert!(iter.next().is_none());
}

/// Asserts that spawning at a retired index fails while another vacant slot stays recyclable.
#[test]
fn spawn_retired() {
    let mut entities = Entities::new();
    let exhausted = EntityId::new_from_index_and_gen(0, EntityId::max_gen() - 1);
    assert!(entities.spawn(exhausted));
    let recyclable = entities.generate();

    assert!(entities.delete_unchecked(exhausted));
    assert!(entities.delete_unchecked(recyclable));
    assert!(!entities.spawn(exhausted));
    assert!(!entities.spawn(EntityId::new_from_index_and_gen(0, EntityId::max_gen())));
    assert_eq!(entities.generate(), EntityId::new_from_index_and_gen(1, 1));
}

/// Asserts that clearing an allocator containing only retired slots emits no callbacks and leaves
/// the next allocation to append a fresh slot.
#[test]
fn clear_retired() {
    let mut entities = Entities::new();
    let exhausted = EntityId::new_from_index_and_gen(0, EntityId::max_gen() - 1);
    assert!(entities.spawn(exhausted));
    assert!(entities.delete_unchecked(exhausted));
    entities.on_deletion(|id| panic!("unexpected clear callback for {id:?}"));

    Storage::clear(&mut entities, TrackingTimestamp::origin());

    assert_eq!(entities.iter().next(), None);
    assert_eq!(entities.list, None);
    assert_eq!(entities.generate(), EntityId::new(1));
}

/// Asserts that clearing retires a vacant slot whose generation can no longer advance and keeps
/// the other slots recyclable in index order.
#[test]
fn clear_exhausted_hole() {
    let mut entities = Entities::new();
    let first = entities.generate();
    let almost_exhausted = EntityId::new_from_index_and_gen(1, EntityId::max_gen() - 2);
    assert!(entities.spawn(almost_exhausted));
    let last = entities.generate();
    assert!(entities.delete_unchecked(almost_exhausted));

    Storage::clear(&mut entities, TrackingTimestamp::origin());

    assert_eq!(entities.iter().next(), None);
    assert_eq!(entities.data[1], EntityId::dead());
    assert_eq!(
        entities.generate(),
        EntityId::new_from_index_and_gen(first.index(), 1)
    );
    assert_eq!(
        entities.generate(),
        EntityId::new_from_index_and_gen(last.index(), 1)
    );
    assert_eq!(entities.generate(), EntityId::new(3));
}
