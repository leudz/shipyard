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
