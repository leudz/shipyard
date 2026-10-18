//! Resolver edge cases reached through normal query selection.
//!
//! Coverage, rejecting segments, held mutable outputs and size hints are exercised
//! by grouping_automatic, grouping_nested and the public iteration suite. Unequal
//! range starts and partial trust remain isolated in grouping_regions.

use super::grouping_fixtures::{A, B};
use crate::{track, Component, IntoIter, View, World};
use alloc::vec::Vec;

struct C(u32);
impl Component for C {
    type Tracking = track::Untracked;
}

#[test]
fn captain_positions() {
    let mut world = World::new_for_test();
    let ids: Vec<_> = (0..4).map(|i| world.add_entity((A(i), B(i)))).collect();
    for i in [0, 2, 3] {
        world.add_component(ids[i], C(i as u32));
    }
    let (a, b, c) = world
        .borrow::<(View<'_, A>, View<'_, B>, View<'_, C>)>()
        .unwrap();
    macro_rules! check {
        ($query:expr, $captain:expr) => {{
            let iter = $query.iter();
            assert_eq!(iter.shiperator.single_captain, $captain);
            assert_eq!(iter.shiperator.captain_mask, 1 << $captain);
            let found: Vec<_> = iter
                .with_id()
                .map(|(id, (a, b, c))| {
                    assert_eq!(a.0, b.0);
                    assert_eq!(b.0, c.0);
                    id
                })
                .collect();
            assert_eq!(found, [ids[0], ids[2], ids[3]]);
        }};
    }
    check!((&c, &a, &b), 0);
    check!((&a, &c, &b), 1);
    check!((&a, &b, &c), 2);
}

#[cfg(feature = "extended_tuple")]
#[test]
fn captain_32() {
    use crate::iter::mixed::NO_SINGLE_CAPTAIN;
    // Distinct components allow a real 32-member group; repeated views cannot
    // establish group membership and used to hide that distinction in this test.
    struct Member<const N: usize>(u32);
    impl<const N: usize> Component for Member<N> {
        type Tracking = track::Untracked;
    }
    macro_rules! check {
        ($($n:tt),+) => {{
            let mut world = World::new_for_test();
            let ids: Vec<_> = (0..3).map(|i| world.add_entity(($(Member::<$n>(i),)+))).collect();
            // Before grouping, make the final child the cheapest ordinary captain.
            $(if $n != 31 { world.add_entity(Member::<$n>(100)); })+
            {
                let views = world.borrow::<($(View<'_, Member<$n>>,)+)>().unwrap();
                let iter = ($(&views.$n,)+).iter();
                assert_eq!(iter.shiperator.single_captain, 31);
                assert_eq!(iter.shiperator.captain_mask, 1 << 31);
                assert_eq!(iter.with_id().map(|(id, items)| {
                    $(assert_eq!(items.$n.0, items.31.0);)+
                    id
                }).collect::<Vec<_>>(), ids);
            }
            world.group::<($(Member<$n>,)+)>();
            let views = world.borrow::<($(View<'_, Member<$n>>,)+)>().unwrap();
            let iter = ($(&views.$n,)+).iter();
            assert_eq!(iter.shiperator.captain_mask, u32::MAX);
            assert_eq!(iter.shiperator.single_captain, NO_SINGLE_CAPTAIN);
            assert_eq!(iter.size_hint(), (3, Some(3)));
            assert_eq!(iter.with_id().fold(Vec::new(), |mut found, (id, items)| {
                $(assert_eq!(items.$n.0, items.31.0);)+
                found.push(id);
                found
            }), ids);
        }};
    }
    check!(
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31
    );
}
