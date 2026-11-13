use super::matched::{crossing, crossing_groups};
use super::*;

/// Checks the resulting plan, memberships, full trust, sparse mapping and preserved payloads.
fn assert_paired(declared: &[&[StorageId]], build: impl FnOnce() -> World) {
    let mut world = build();
    regroup_checked(&mut world, declared);
}

/// Returns an empty world grouping AB, with 64 AB entities added in bulk.
pub(super) fn bulk() -> World {
    let mut world = World::builder_for_test().with_group::<(A, B)>().build();
    world
        .bulk_add_entity((0..64).map(|i| (A(i), B(i))))
        .for_each(drop);

    world
}

/// Returns [`pairs`] with lone A entities, regrouped, then 32 AB entities added in bulk.
/// AB's range grows over A's ungrouped rows.
fn bulk_existing() -> World {
    let (mut world, _) = pairs(32);
    for i in 32..40 {
        world.add_entity(A(i));
    }
    world.regroup();
    world
        .bulk_add_entity((100..132).map(|i| (A(i), B(i))))
        .for_each(drop);

    world
}

/// Asserts that AB entities added in bulk to an empty world preserve their payloads and join AB.
#[test]
fn paired_bulk() {
    assert_paired(&[&ab()], bulk);
}

/// Asserts that AB entities added in bulk next to existing ranges and ungrouped rows get the
/// expected memberships and preserved payloads.
#[test]
fn paired_bulk_existing() {
    assert_paired(&[&ab()], bulk_existing);
}

/// Asserts that AB entities added one by one get the expected memberships and preserved payloads.
#[test]
fn paired_one_by_one() {
    assert_paired(&[&ab()], || {
        let (mut world, _) = pairs(16);
        for i in 100..116 {
            world.add_entity((A(i), B(i)));
        }

        world
    });
}

/// Asserts that lone A entities between AB additions stay ungrouped and don't mispair later rows.
#[test]
fn paired_one_member() {
    assert_paired(&[&ab()], || {
        let (mut world, _) = pairs(16);
        for i in 100..132 {
            if i % 3 == 0 {
                world.add_entity(A(i));
            } else {
                world.add_entity((A(i), B(i)));
            }
        }

        world
    });
}

/// Asserts that restoring B promotes the ungrouped A entities and preserves their replacement B values.
#[test]
fn paired_missing_member() {
    assert_paired(&[&ab()], || {
        let (mut world, entities) = pairs(64);
        for &entity in entities.iter().step_by(3) {
            world.remove::<(B,)>(entity);
        }
        world.regroup();
        for (i, &entity) in (100..).zip(&entities).step_by(3) {
            world.add_component(entity, B(i));
        }

        world
    });
}

/// Asserts that reverse-order B additions classify correctly when suffix offsets don't pair.
#[test]
fn paired_interleaved() {
    assert_paired(&[&ab()], || {
        let (mut world, _) = pairs(16);
        let entities: Vec<EntityId> = (100..124).map(|i| world.add_entity(A(i))).collect();
        for (i, &entity) in (200..224).zip(&entities).rev() {
            world.add_component(entity, B(i));
        }

        world
    });
}

/// Asserts that with AB and AC, where no storage is alone, new A, AB, AC and ABC entities get the
/// expected memberships and preserved payloads.
#[test]
fn paired_crossing() {
    let [ab, ac] = crossing_groups();
    assert_paired(&[&ab, &ac], || {
        let (mut world, _) = crossing(32);
        world.add_entity(A(100));
        world.add_entity((A(101), B(101)));
        world.add_entity((A(102), C));
        world.add_entity((A(103), B(103), C));

        world
    });
}
