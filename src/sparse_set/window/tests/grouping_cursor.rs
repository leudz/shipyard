use super::super::EntityCursor;
use crate::EntityId;
use alloc::vec::Vec;
use core::ptr::NonNull;

fn entities(len: u64) -> Vec<EntityId> {
    (0..len).map(EntityId::new).collect()
}

fn cursor(entities: &[EntityId]) -> EntityCursor {
    // SAFE: `entities` is a live slice that outlives every use of the cursor in these tests
    // and isn't modified meanwhile.
    unsafe { EntityCursor::new(NonNull::from(entities).cast(), entities.len()) }
}

/// Asserts that empty cursors report no entity and never move,
/// whether built empty, from a dangling pointer or from an empty slice.
#[test]
fn empty() {
    // SAFE: `len` is 0 so the pointer is never read.
    let from_dangling = unsafe { EntityCursor::new(NonNull::dangling(), 0) };
    let from_slice = cursor(&[]);

    for mut cursor in [EntityCursor::empty(), from_dangling, from_slice] {
        assert!(cursor.is_empty());
        assert_eq!(cursor.len(), 0);
        assert!(!cursor.advance());
        assert_eq!(cursor.local_index(), 0);
    }
}

/// Asserts that a single entity cursor yields its entity once then stays exhausted.
#[test]
fn one() {
    let entities = entities(1);
    let mut cursor = cursor(&entities);

    assert_eq!(cursor.len(), 1);
    // SAFE: The cursor holds one entity.
    assert_eq!(unsafe { cursor.current() }, entities[0]);

    assert!(cursor.advance());
    assert!(cursor.is_empty());
    assert_eq!(cursor.local_index(), 1);
}

/// Asserts that advancing an exhausted cursor repeatedly doesn't move it.
#[test]
fn exhausted_advance() {
    let entities = entities(2);
    let mut cursor = cursor(&entities);

    assert!(cursor.advance());
    assert!(cursor.advance());
    for _ in 0..3 {
        assert!(!cursor.advance());
        assert_eq!(cursor.len(), 0);
        assert_eq!(cursor.local_index(), 2);
    }
}

/// Asserts that a partially consumed cursor reads the next unconsumed entity
/// and reports its local index and remaining length.
#[test]
fn partial() {
    let entities = entities(5);
    let mut cursor = cursor(&entities);

    cursor.advance();
    cursor.advance();

    assert_eq!(cursor.len(), 3);
    assert_eq!(cursor.local_index(), 2);
    // SAFE: 3 entities are left.
    assert_eq!(unsafe { cursor.current() }, entities[2]);
}

/// Asserts that splitting a range whose next local index is 3 keeps the original
/// local indices on both halves instead of restarting the right one at zero.
#[test]
fn split_offsets() {
    let entities = entities(8);
    let mut cursor = cursor(&entities);
    for _ in 0..3 {
        cursor.advance();
    }

    let (left, right) = cursor.split_at(2);

    assert_eq!((left.local_index(), left.len()), (3, 2));
    assert_eq!((right.local_index(), right.len()), (5, 3));
    // SAFE: Both halves are nonempty.
    unsafe {
        assert_eq!(left.current(), entities[3]);
        assert_eq!(right.current(), entities[5]);
    }
}

/// Asserts that splitting at either end gives an empty half and a half with every entity,
/// both keeping the right local indices.
#[test]
fn split_ends() {
    let entities = entities(3);
    let mut cursor = cursor(&entities);
    cursor.advance();

    let (left, right) = cursor.split_at(0);
    assert_eq!((left.local_index(), left.len()), (1, 0));
    assert_eq!((right.local_index(), right.len()), (1, 2));

    let (left, right) = cursor.split_at(2);
    assert_eq!((left.local_index(), left.len()), (1, 2));
    assert_eq!((right.local_index(), right.len()), (3, 0));
    assert!(right.is_empty());
}

/// Asserts that splitting past the unconsumed entities panics instead of creating
/// an out of bounds cursor.
#[test]
#[should_panic(expected = "split index out of bounds")]
fn split_out_of_bounds() {
    let entities = entities(3);
    let mut cursor = cursor(&entities);
    cursor.advance();

    let _ = cursor.split_at(3);
}

/// Asserts that a segment taken from a partially consumed cursor starts after the
/// skipped entities and restarts local indices at 0.
#[test]
fn segment() {
    let entities = entities(6);
    let mut cursor = cursor(&entities);
    cursor.advance();

    let segment = cursor.segment(2, 3);

    assert_eq!((segment.local_index(), segment.len()), (0, 3));
    // SAFE: The segment holds 3 entities.
    assert_eq!(unsafe { segment.current() }, entities[3]);
}

/// Asserts that a segment ending past the unconsumed entities panics instead of creating
/// an out of bounds cursor.
#[test]
#[should_panic(expected = "segment out of bounds")]
fn segment_out_of_bounds() {
    let entities = entities(4);

    let _ = cursor(&entities).segment(2, 3);
}

/// Asserts that a segment whose end overflows panics instead of wrapping.
#[test]
#[should_panic(expected = "segment out of bounds")]
fn segment_overflow() {
    let entities = entities(4);

    let _ = cursor(&entities).segment(1, usize::MAX);
}
