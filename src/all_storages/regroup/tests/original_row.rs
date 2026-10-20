use super::super::original_row;
use super::*;

/// Asserts that an entity's original row is its component's row, and that an entity without
/// a component or a deleted entity whose index was reused has none.
#[test]
fn generations() {
    let mut world = World::new_for_test();
    let first = world.add_entity((A(0),));
    let deleted = world.add_entity((A(1),));
    let without = world.add_entity((B(2),));
    world.delete_entity(deleted);
    let reused = world.add_entity((A(3),));
    assert_eq!(reused.index(), deleted.index());

    let a = world.borrow::<View<'_, A>>().unwrap();
    let sparse = a.sparse_array().unwrap();

    assert_eq!(original_row(sparse, first), a.index_of(first));
    assert_eq!(original_row(sparse, reused), a.index_of(reused));
    assert_eq!(original_row(sparse, deleted), None);
    assert_eq!(original_row(sparse, without), None);
}
