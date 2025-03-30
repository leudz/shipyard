use shipyard::sparse_set::SparseSet;
use shipyard::*;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct I16(i16);
impl Component for I16 {
    type Tracking = track::Untracked;
}

#[test]
fn basic() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32>, ViewMut<I16>)>()
        .unwrap();

    entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    entities.add_entity(&mut u32s, U32(1));
    entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));
    entities.add_entity((&mut u32s, &mut i16s), (U32(4), I16(14)));

    let mut iter = (&u32s).iter();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    assert_eq!(iter.next().unwrap(), &U32(0));
    assert_eq!(iter.next().unwrap(), &U32(1));
    assert_eq!(iter.next().unwrap(), &U32(2));
    assert_eq!(iter.next().unwrap(), &U32(4));
    assert!(iter.next().is_none());

    let mut iter = (&mut u32s).iter();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    assert_eq!(*iter.next().unwrap(), U32(0));
    assert_eq!(*iter.next().unwrap(), U32(1));
    assert_eq!(*iter.next().unwrap(), U32(2));
    assert_eq!(*iter.next().unwrap(), U32(4));
    assert!(iter.next().is_none());

    let mut iter = (&i16s).iter();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    assert_eq!(iter.next().unwrap(), &I16(10));
    assert_eq!(iter.next().unwrap(), &I16(12));
    assert_eq!(iter.next().unwrap(), &I16(13));
    assert_eq!(iter.next().unwrap(), &I16(14));
    assert!(iter.next().is_none());

    let mut iter = (&mut i16s).iter();
    assert_eq!(iter.size_hint(), (4, Some(4)));
    assert!(*iter.next().unwrap() == I16(10));
    assert!(*iter.next().unwrap() == I16(12));
    assert!(*iter.next().unwrap() == I16(13));
    assert!(*iter.next().unwrap() == I16(14));
    assert!(iter.next().is_none());

    let mut iter = (&u32s, &i16s).iter();
    assert_eq!(iter.size_hint(), (0, Some(4)));
    assert_eq!(iter.next().unwrap(), (&U32(0), &I16(10)));
    assert_eq!(iter.next().unwrap(), (&U32(2), &I16(12)));
    assert_eq!(iter.next().unwrap(), (&U32(4), &I16(14)));
    assert!(iter.next().is_none());

    let mut iter = (&mut u32s, &mut i16s).iter();
    assert_eq!(iter.size_hint(), (0, Some(4)));
    assert_eq!(
        iter.next().map(|(x, y)| (*x, *y)).unwrap(),
        (U32(0), I16(10))
    );
    assert_eq!(
        iter.next().map(|(x, y)| (*x, *y)).unwrap(),
        (U32(2), I16(12))
    );
    assert_eq!(
        iter.next().map(|(x, y)| (*x, *y)).unwrap(),
        (U32(4), I16(14))
    );
    assert!(iter.next().is_none());

    let mut iter = (&i16s, &u32s).iter();
    assert_eq!(iter.size_hint(), (0, Some(4)));
    assert_eq!(iter.next().unwrap(), (&I16(10), &U32(0)));
    assert_eq!(iter.next().unwrap(), (&I16(12), &U32(2)));
    assert_eq!(iter.next().unwrap(), (&I16(14), &U32(4)));
    assert!(iter.next().is_none());

    let mut iter = (&mut i16s, &mut u32s).iter();
    assert_eq!(iter.size_hint(), (0, Some(4)));
    assert_eq!(
        iter.next().map(|(x, y)| (*x, *y)).unwrap(),
        (I16(10), U32(0))
    );
    assert_eq!(
        iter.next().map(|(x, y)| (*x, *y)).unwrap(),
        (I16(12), U32(2))
    );
    assert_eq!(
        iter.next().map(|(x, y)| (*x, *y)).unwrap(),
        (I16(14), U32(4))
    );
    assert!(iter.next().is_none());
}

#[test]
fn with_id() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32>, ViewMut<I16>)>()
        .unwrap();

    let id0 = entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    let id1 = entities.add_entity(&mut u32s, U32(1));
    let id2 = entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    let id3 = entities.add_entity(&mut i16s, I16(13));
    let id4 = entities.add_entity((&mut u32s, &mut i16s), (U32(4), I16(14)));

    let mut iter = (&u32s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, &U32(0)));
    assert_eq!(iter.next().unwrap(), (id1, &U32(1)));
    assert_eq!(iter.next().unwrap(), (id2, &U32(2)));
    assert_eq!(iter.next().unwrap(), (id4, &U32(4)));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s).iter().with_id();
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id0, U32(0)));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id1, U32(1)));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id2, U32(2)));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id4, U32(4)));
    assert!(iter.next().is_none());

    let mut iter = i16s.iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, &I16(10)));
    assert_eq!(iter.next().unwrap(), (id2, &I16(12)));
    assert_eq!(iter.next().unwrap(), (id3, &I16(13)));
    assert_eq!(iter.next().unwrap(), (id4, &I16(14)));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s).iter().with_id();
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id0, I16(10)));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id2, I16(12)));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id3, I16(13)));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id4, I16(14)));
    assert!(iter.next().is_none());

    let mut iter = (&u32s, &i16s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, (&U32(0), &I16(10))));
    assert_eq!(iter.next().unwrap(), (id2, (&U32(2), &I16(12))));
    assert_eq!(iter.next().unwrap(), (id4, (&U32(4), &I16(14))));
    assert!(iter.next().is_none());
    let mut iter = (&mut u32s, &mut i16s).iter().with_id();
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id0, (U32(0), I16(10)))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id2, (U32(2), I16(12)))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id4, (U32(4), I16(14)))
    );
    assert!(iter.next().is_none());

    let mut iter = (&i16s, &u32s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, (&I16(10), &U32(0))));
    assert_eq!(iter.next().unwrap(), (id2, (&I16(12), &U32(2))));
    assert_eq!(iter.next().unwrap(), (id4, (&I16(14), &U32(4))));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s, &mut u32s).iter().with_id();
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id0, (I16(10), U32(0)))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id2, (I16(12), U32(2)))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id4, (I16(14), U32(4)))
    );
    assert!(iter.next().is_none());
}

#[test]
fn empty() {
    let world = World::new();

    let (usizes, u32s) = world.borrow::<(ViewMut<USIZE>, ViewMut<U32>)>().unwrap();

    assert!(u32s.iter().next().is_none());
    assert!(u32s.iter().with_id().next().is_none());
    assert!(usizes.iter().next().is_none());
    assert!(usizes.iter().with_id().next().is_none());
    assert!((&usizes, &u32s).iter().next().is_none());
    assert!((&usizes, &u32s).iter().with_id().next().is_none());
}

#[test]
fn not() {
    let world = World::new();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32>, ViewMut<I16>)>()
        .unwrap();

    entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    entities.add_entity(&mut u32s, U32(1));
    entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));
    entities.add_entity((&mut u32s, &mut i16s), (U32(4), I16(14)));

    let mut iter = (&u32s, !&i16s).iter();
    assert_eq!(iter.size_hint(), (0, Some(4)));
    assert_eq!(iter.next().unwrap(), (&U32(1), ()));
    assert!(iter.next().is_none());
}

#[test]
fn or() {
    let mut world = World::new();

    world.add_entity((U32(0), I16(10)));
    world.add_entity((U32(1),));
    world.add_entity((U32(2), I16(12)));
    world.add_entity((I16(13),));
    world.add_entity((U32(4), I16(14), USIZE(24)));
    world.add_entity((I16(15), USIZE(25)));

    world.bulk_add_entity((0..100).map(|_| (USIZE(0),)));

    let (u32s, i16s, usizes) = world
        .borrow::<(View<U32>, View<I16>, View<USIZE>)>()
        .unwrap();

    let mut iter = (&u32s | &i16s, &usizes).iter();
    assert_eq!(iter.next().unwrap(), (OneOfTwo::One(&U32(4)), &USIZE(24)));
    dbg!("now");
    assert_eq!(iter.next().unwrap(), (OneOfTwo::Two(&I16(15)), &USIZE(25)));
    assert!(iter.next().is_none());
}

/// Makes sure storages are borrowed while the iterator from `World::iter` is alive.
#[test]
fn world_iter_correct_borrow() {
    let mut world = World::new();

    world.add_entity(USIZE(0));

    let mut iter = world.iter::<&USIZE>();
    let us = iter.into_iter().collect::<Vec<_>>();

    let usizes = world.borrow::<ViewMut<USIZE>>();
    assert_eq!(
        usizes.err(),
        Some(error::GetStorage::StorageBorrow {
            name: Some(core::any::type_name::<SparseSet<USIZE>>()),
            id: StorageId::of::<SparseSet<USIZE>>(),
            borrow: error::Borrow::Unique
        })
    );

    for u in us {
        dbg!(u);
    }
}

// #[test]
// fn chunk() {
//     let mut world = World::new();

//     world.bulk_add_entity((0..100).map(|_| (1u32,)));

//     let u32s = world.borrow::<View<U32>>().unwrap();

//     assert_eq!(
//         u32s.fast_iter()
//             .into_chunk(8)
//             .ok()
//             .unwrap()
//             .flatten()
//             .count(),
//         100
//     );

//     let mut i = 0;
//     for n in u32s.fast_iter().into_chunk(1).ok().unwrap().flatten() {
//         i += n;
//     }
//     assert_eq!(i, 100);

//     let mut iter = u32s.fast_iter().into_chunk_exact(8).ok().unwrap();

//     assert_eq!(
//         iter.remainder().iter().count() + iter.flatten().count(),
//         100
//     );

//     let mut iter = u32s.fast_iter().into_chunk_exact(1).ok().unwrap();

//     let mut i = 0;
//     for n in iter.remainder() {
//         i += n;
//     }
//     for n in iter.flatten() {
//         i += n;
//     }
//     assert_eq!(i, 100);
// }

#[test]
fn vec() {
    let mut world = World::new();

    world.bulk_add_entity((0..100).map(USIZE));

    world.run(|v_usize: View<USIZE>| {
        let eids = v_usize.iter().ids().collect::<Vec<_>>();

        let other_eids = (&*eids, &v_usize)
            .iter()
            .map(|(eid, _a)| eid)
            .collect::<Vec<_>>();

        assert_eq!(eids.len(), 100);
        assert_eq!(eids, other_eids);
    });
}
