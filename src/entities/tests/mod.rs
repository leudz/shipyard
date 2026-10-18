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
