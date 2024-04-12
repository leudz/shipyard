use shipyard::*;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct U64(u64);
impl Component for U64 {
    type Tracking = track::Untracked;
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct I16(i16);
impl Component for I16 {
    type Tracking = track::Untracked;
}

#[test]
fn basic() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u64s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)>()
        .unwrap();

    entities.add_entity((&mut u64s, &mut i16s), (U64(0), I16(10)));
    entities.add_entity(&mut u64s, U64(1));
    entities.add_entity((&mut u64s, &mut i16s), (U64(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));
    entities.add_entity((&mut u64s, &mut i16s), (U64(4), I16(14)));

    if let iter::Iter::Tight(mut iter) = (&u64s).iter() {
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert_eq!(iter.next().unwrap(), &U64(0));
        assert_eq!(iter.next().unwrap(), &U64(1));
        assert_eq!(iter.next().unwrap(), &U64(2));
        assert_eq!(iter.next().unwrap(), &U64(4));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    if let iter::Iter::Tight(mut iter) = (&mut u64s).iter() {
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert_eq!(*iter.next().unwrap(), U64(0));
        assert_eq!(*iter.next().unwrap(), U64(1));
        assert_eq!(*iter.next().unwrap(), U64(2));
        assert_eq!(*iter.next().unwrap(), U64(4));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    if let iter::Iter::Tight(mut iter) = (&i16s).iter() {
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert_eq!(iter.next().unwrap(), &I16(10));
        assert_eq!(iter.next().unwrap(), &I16(12));
        assert_eq!(iter.next().unwrap(), &I16(13));
        assert_eq!(iter.next().unwrap(), &I16(14));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    if let iter::Iter::Tight(mut iter) = (&mut i16s).iter() {
        assert_eq!(iter.size_hint(), (4, Some(4)));
        assert!(*iter.next().unwrap() == I16(10));
        assert!(*iter.next().unwrap() == I16(12));
        assert!(*iter.next().unwrap() == I16(13));
        assert!(*iter.next().unwrap() == I16(14));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    if let iter::Iter::Mixed(mut iter) = (&u64s, &i16s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(iter.next().unwrap(), (&U64(0), &I16(10)));
        assert_eq!(iter.next().unwrap(), (&U64(2), &I16(12)));
        assert_eq!(iter.next().unwrap(), (&U64(4), &I16(14)));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    if let iter::Iter::Mixed(mut iter) = (&mut u64s, &mut i16s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(
            iter.next().map(|(x, y)| (*x, *y)).unwrap(),
            (U64(0), I16(10))
        );
        assert_eq!(
            iter.next().map(|(x, y)| (*x, *y)).unwrap(),
            (U64(2), I16(12))
        );
        assert_eq!(
            iter.next().map(|(x, y)| (*x, *y)).unwrap(),
            (U64(4), I16(14))
        );
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    if let iter::Iter::Mixed(mut iter) = (&i16s, &u64s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(iter.next().unwrap(), (&I16(10), &U64(0)));
        assert_eq!(iter.next().unwrap(), (&I16(12), &U64(2)));
        assert_eq!(iter.next().unwrap(), (&I16(14), &U64(4)));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }

    if let iter::Iter::Mixed(mut iter) = (&mut i16s, &mut u64s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(
            iter.next().map(|(x, y)| (*x, *y)).unwrap(),
            (I16(10), U64(0))
        );
        assert_eq!(
            iter.next().map(|(x, y)| (*x, *y)).unwrap(),
            (I16(12), U64(2))
        );
        assert_eq!(
            iter.next().map(|(x, y)| (*x, *y)).unwrap(),
            (I16(14), U64(4))
        );
        assert!(iter.next().is_none());
    } else {
        panic!()
    }
}

#[test]
fn with_id() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u64s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)>()
        .unwrap();

    let id0 = entities.add_entity((&mut u64s, &mut i16s), (U64(0), I16(10)));
    let id1 = entities.add_entity(&mut u64s, U64(1));
    let id2 = entities.add_entity((&mut u64s, &mut i16s), (U64(2), I16(12)));
    let id3 = entities.add_entity(&mut i16s, I16(13));
    let id4 = entities.add_entity((&mut u64s, &mut i16s), (U64(4), I16(14)));

    let mut iter = (&u64s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, &U64(0)));
    assert_eq!(iter.next().unwrap(), (id1, &U64(1)));
    assert_eq!(iter.next().unwrap(), (id2, &U64(2)));
    assert_eq!(iter.next().unwrap(), (id4, &U64(4)));
    assert!(iter.next().is_none());
    let mut iter = (&mut u64s).iter().with_id();
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id0, U64(0)));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id1, U64(1)));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id2, U64(2)));
    assert_eq!(iter.next().map(|(id, y)| (id, *y)).unwrap(), (id4, U64(4)));
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

    let mut iter = (&u64s, &i16s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, (&U64(0), &I16(10))));
    assert_eq!(iter.next().unwrap(), (id2, (&U64(2), &I16(12))));
    assert_eq!(iter.next().unwrap(), (id4, (&U64(4), &I16(14))));
    assert!(iter.next().is_none());
    let mut iter = (&mut u64s, &mut i16s).iter().with_id();
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id0, (U64(0), I16(10)))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id2, (U64(2), I16(12)))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id4, (U64(4), I16(14)))
    );
    assert!(iter.next().is_none());

    let mut iter = (&i16s, &u64s).iter().with_id();
    assert_eq!(iter.next().unwrap(), (id0, (&I16(10), &U64(0))));
    assert_eq!(iter.next().unwrap(), (id2, (&I16(12), &U64(2))));
    assert_eq!(iter.next().unwrap(), (id4, (&I16(14), &U64(4))));
    assert!(iter.next().is_none());
    let mut iter = (&mut i16s, &mut u64s).iter().with_id();
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id0, (I16(10), U64(0)))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id2, (I16(12), U64(2)))
    );
    assert_eq!(
        iter.next().map(|(id, (x, y))| (id, (*x, *y))).unwrap(),
        (id4, (I16(14), U64(4)))
    );
    assert!(iter.next().is_none());
}

#[test]
fn empty() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (usizes, u64s) = world.borrow::<(ViewMut<USIZE>, ViewMut<U64>)>().unwrap();

    assert!(u64s.iter().next().is_none());
    assert!(u64s.iter().with_id().next().is_none());
    assert!(usizes.iter().next().is_none());
    assert!(usizes.iter().with_id().next().is_none());
    assert!((&usizes, &u64s).iter().next().is_none());
    assert!((&usizes, &u64s).iter().with_id().next().is_none());
}

#[test]
fn not() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u64s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)>()
        .unwrap();

    entities.add_entity((&mut u64s, &mut i16s), (U64(0), I16(10)));
    entities.add_entity(&mut u64s, U64(1));
    entities.add_entity((&mut u64s, &mut i16s), (U64(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));
    entities.add_entity((&mut u64s, &mut i16s), (U64(4), I16(14)));

    if let iter::Iter::Mixed(mut iter) = (&u64s, !&i16s).iter() {
        assert_eq!(iter.size_hint(), (0, Some(4)));
        assert_eq!(iter.next().unwrap(), (&U64(1), ()));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }
}

#[test]
fn iter_by() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    let (mut entities, mut u64s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U64>, ViewMut<I16>)>()
        .unwrap();

    entities.add_entity((&mut u64s, &mut i16s), (U64(0), I16(10)));
    entities.add_entity(&mut u64s, U64(1));
    entities.add_entity((&mut u64s, &mut i16s), (U64(2), I16(12)));
    entities.add_entity((&mut u64s, &mut i16s), (U64(4), I16(14)));

    u64s.sort_unstable_by(|x, y| x.0.cmp(&y.0).reverse());

    let mut iter = (&u64s, &i16s).iter_by::<U64>();
    assert_eq!(iter.next(), Some((&U64(4), &I16(14))));
    assert_eq!(iter.next(), Some((&U64(2), &I16(12))));
    assert_eq!(iter.next(), Some((&U64(0), &I16(10))));
    assert_eq!(iter.next(), None);

    let mut iter = (&i16s, &u64s).iter_by::<U64>();
    assert_eq!(iter.next(), Some((&I16(14), &U64(4))));
    assert_eq!(iter.next(), Some((&I16(12), &U64(2))));
    assert_eq!(iter.next(), Some((&I16(10), &U64(0))));
    assert_eq!(iter.next(), None);

    let mut iter = (&u64s, &i16s).iter_by::<I16>();
    assert_eq!(iter.next(), Some((&U64(0), &I16(10))));
    assert_eq!(iter.next(), Some((&U64(2), &I16(12))));
    assert_eq!(iter.next(), Some((&U64(4), &I16(14))));
    assert_eq!(iter.next(), None);

    let mut iter = (&i16s, &u64s).iter_by::<I16>();
    assert_eq!(iter.next(), Some((&I16(10), &U64(0))));
    assert_eq!(iter.next(), Some((&I16(12), &U64(2))));
    assert_eq!(iter.next(), Some((&I16(14), &U64(4))));
    assert_eq!(iter.next(), None);
}

#[test]
fn or() {
    let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

    world.add_entity((U64(0), I16(10)));
    world.add_entity((U64(1),));
    world.add_entity((U64(2), I16(12)));
    world.add_entity((I16(13),));
    world.add_entity((U64(4), I16(14), USIZE(24)));
    world.add_entity((I16(15), USIZE(25)));

    world.bulk_add_entity((0..100).map(|_| (USIZE(0),)));

    let (u64s, i16s, usizes) = world
        .borrow::<(View<U64>, View<I16>, View<USIZE>)>()
        .unwrap();

    if let iter::Iter::Mixed(mut iter) = (&u64s | &i16s, &usizes).iter() {
        assert_eq!(iter.next().unwrap(), (OneOfTwo::One(&U64(4)), &USIZE(24)));
        assert_eq!(iter.next().unwrap(), (OneOfTwo::Two(&I16(15)), &USIZE(25)));
        assert!(iter.next().is_none());
    } else {
        panic!()
    }
}

// #[test]
// fn chunk() {
//     let mut world = World::new_with_custom_lock::<parking_lot::RawRwLock>();

//     world.bulk_add_entity((0..100).map(|_| (1u64,)));

//     let u64s = world.borrow::<View<U64>>().unwrap();

//     assert_eq!(
//         u64s.fast_iter()
//             .into_chunk(8)
//             .ok()
//             .unwrap()
//             .flatten()
//             .count(),
//         100
//     );

//     let mut i = 0;
//     for n in u64s.fast_iter().into_chunk(1).ok().unwrap().flatten() {
//         i += n;
//     }
//     assert_eq!(i, 100);

//     let mut iter = u64s.fast_iter().into_chunk_exact(8).ok().unwrap();

//     assert_eq!(
//         iter.remainder().iter().count() + iter.flatten().count(),
//         100
//     );

//     let mut iter = u64s.fast_iter().into_chunk_exact(1).ok().unwrap();

//     let mut i = 0;
//     for n in iter.remainder() {
//         i += n;
//     }
//     for n in iter.flatten() {
//         i += n;
//     }
//     assert_eq!(i, 100);
// }
