use super::*;

#[test]
fn serde_json() {
    let string = serde_json::to_string(&EntityId::new_from_index_and_gen(10, 2)).unwrap();
    assert_eq!(r#"{"index":10,"gen":2}"#, string);

    let entity = serde_json::de::from_str::<EntityId>(&string).unwrap();
    assert_eq!(entity, EntityId::new_from_index_and_gen(10, 2));
}

#[test]
fn bincode() {
    let bytes = bincode::serialize(&EntityId::new_from_parts(10, 2)).unwrap();
    assert_eq!(&[10, 0, 0, 0, 0, 0, 2, 0][..], &bytes);

    let entity = bincode::deserialize::<EntityId>(&bytes).unwrap();
    assert_eq!(entity, EntityId::new_from_parts(10, 2));
}

/// Asserts that the dead sentinel keeps its wire value and decodes back to a dead ID.
#[test]
fn bincode_dead() {
    let dead = EntityId::dead();
    let bytes = bincode::serialize(&dead).unwrap();
    assert_eq!(bytes, (u64::MAX - 1).to_le_bytes());
    assert_eq!(bincode::deserialize::<EntityId>(&bytes).unwrap(), dead);
}

/// Asserts that the smallest and largest live IDs round-trip through bincode.
#[test]
fn bincode_bounds() {
    for entity in [
        EntityId::new(0),
        EntityId::new_from_parts(EntityId::max_index(), EntityId::max_gen()),
    ] {
        let bytes = bincode::serialize(&entity).unwrap();
        assert_eq!(bincode::deserialize::<EntityId>(&bytes).unwrap(), entity);
    }
}

/// Asserts that the overflowing wire value returns a Serde data error.
#[test]
fn bincode_overflow() {
    let error = bincode::deserialize::<EntityId>(&u64::MAX.to_le_bytes()).unwrap_err();
    assert!(matches!(*error, bincode::ErrorKind::Custom(_)));
}
