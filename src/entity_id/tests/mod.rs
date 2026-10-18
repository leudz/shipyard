use super::*;

#[test]
fn entity_id() {
    let mut entity_id = EntityId::new(0);
    assert_eq!(entity_id.index(), 0);
    assert_eq!(entity_id.gen(), 0);
    entity_id.set_index(701);
    assert_eq!(entity_id.index(), 701);
    assert_eq!(entity_id.gen(), 0);
    entity_id.bump_gen().unwrap();
    entity_id.bump_gen().unwrap();
    entity_id.bump_gen().unwrap();
    assert_eq!(entity_id.index(), 701);
    assert_eq!(entity_id.gen(), 3);
    entity_id.set_index(554);
    assert_eq!(entity_id.index(), 554);
    assert_eq!(entity_id.gen(), 3);
}
