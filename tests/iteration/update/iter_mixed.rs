use shipyard::*;

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

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct M32(u32);
impl Component for M32 {
    type Tracking = track::Modification;
}

/// Asserts that an insertion-only combined filter joins the same values whether it drives or is looked up.
#[test]
fn combined_join_paths() {
    let mut captain_world = World::new();
    captain_world.track_insertion::<U32>();
    captain_world.add_entity((U32(1), I16(1)));
    captain_world
        .borrow::<ViewMut<U32, track::Insertion>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    captain_world.add_entity((U32(2), I16(2)));
    for value in 10..20 {
        captain_world.add_entity((I16(value),));
    }

    let (values, markers) = captain_world
        .borrow::<(View<U32, track::Insertion>, View<I16>)>()
        .unwrap();
    assert_eq!(
        values
            .inserted_or_modified()
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(
        (!values.inserted_or_modified())
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(
        (values.inserted_or_modified(), &markers)
            .iter()
            .map(|(value, marker)| (value.0, marker.0))
            .collect::<Vec<_>>(),
        vec![(2, 2)]
    );
    assert_eq!(
        (!values.inserted_or_modified(), &markers)
            .iter()
            .map(|(value, marker)| (value.0, marker.0))
            .collect::<Vec<_>>(),
        vec![(1, 1)]
    );

    let mut sailor_world = World::new();
    sailor_world.track_insertion::<U32>();
    sailor_world.add_entity((U32(1), I16(1)));
    sailor_world
        .borrow::<ViewMut<U32, track::Insertion>>()
        .unwrap()
        .clear_all_inserted_and_modified();
    for value in 10..20 {
        sailor_world.add_entity((U32(value),));
    }
    sailor_world.add_entity((U32(2), I16(2)));

    let (mut values, markers) = sailor_world
        .borrow::<(ViewMut<U32, track::Insertion>, View<I16>)>()
        .unwrap();
    assert_eq!(
        (values.inserted_or_modified(), &markers)
            .iter()
            .map(|(value, marker)| (value.0, marker.0))
            .collect::<Vec<_>>(),
        vec![(2, 2)]
    );
    assert_eq!(
        (!values.inserted_or_modified(), &markers)
            .iter()
            .map(|(value, marker)| (value.0, marker.0))
            .collect::<Vec<_>>(),
        vec![(1, 1)]
    );
    assert_eq!(
        (values.inserted_or_modified_mut(), &markers)
            .iter()
            .map(|(value, marker)| (value.0, marker.0))
            .collect::<Vec<_>>(),
        vec![(2, 2)]
    );
    assert_eq!(
        (!values.inserted_or_modified_mut(), &markers)
            .iter()
            .map(|(value, marker)| (value.0, marker.0))
            .collect::<Vec<_>>(),
        vec![(1, 1)]
    );
}

/// Asserts that a modification-only combined filter joins and negates without insertion timestamps.
#[test]
fn combined_modification_join() {
    let mut world = World::new();
    world.add_entity((M32(1), I16(1)));
    world.add_entity((M32(2), I16(2)));
    {
        let mut values = world.borrow::<ViewMut<M32>>().unwrap();
        (&mut values).iter().nth(1).unwrap().0 += 10;
    }

    let (mut values, markers) = world.borrow::<(ViewMut<M32>, View<I16>)>().unwrap();
    assert_eq!(
        (values.inserted_or_modified(), &markers)
            .iter()
            .map(|(value, marker)| (value.0, marker.0))
            .collect::<Vec<_>>(),
        vec![(12, 2)]
    );
    assert_eq!(
        (!values.inserted_or_modified(), &markers)
            .iter()
            .map(|(value, marker)| (value.0, marker.0))
            .collect::<Vec<_>>(),
        vec![(1, 1)]
    );
    assert_eq!(
        (values.inserted_or_modified_mut(), &markers)
            .iter()
            .map(|(value, marker)| (value.0, marker.0))
            .collect::<Vec<_>>(),
        vec![(12, 2)]
    );
    assert_eq!(
        (!values.inserted_or_modified_mut(), &markers)
            .iter()
            .map(|(value, marker)| (value.0, marker.0))
            .collect::<Vec<_>>(),
        vec![(1, 1)]
    );
}

#[test]
fn basic() {
    let mut world = World::new();
    world.track_all::<U32>();

    world.run(
        |(mut entities, mut u32s, mut i16s): (
            EntitiesViewMut,
            ViewMut<U32, track::All>,
            ViewMut<I16>,
        )| {
            entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
            entities.add_entity(&mut u32s, U32(1));
            entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
            entities.add_entity(&mut i16s, I16(13));

            assert_eq!(u32s.inserted().iter().count(), 3);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 3);

            u32s.clear_all_inserted();
        },
    );

    world.run(
        |(mut entities, mut u32s, mut i16s): (
            EntitiesViewMut,
            ViewMut<U32, track::All>,
            ViewMut<I16>,
        )| {
            assert_eq!(u32s.inserted().iter().count(), 0);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 0);

            entities.add_entity((&mut u32s, &mut i16s), (U32(4), I16(14)));

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 1);

            let mut iter = (&u32s).iter();
            assert_eq!(iter.size_hint(), (4, Some(4)));
            assert_eq!(iter.next().unwrap(), &U32(0));
            assert_eq!(iter.next().unwrap(), &U32(1));
            assert_eq!(iter.next().unwrap(), &U32(2));
            assert_eq!(iter.next().unwrap(), &U32(4));
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 1);

            let mut iter = (&mut u32s).iter();
            assert_eq!(iter.size_hint(), (4, Some(4)));
            let mut next = iter.next().unwrap();
            next.0 += 1;
            next.0 -= 1;
            assert_eq!(*next, U32(0));
            assert_eq!(*iter.next().unwrap(), U32(1));
            assert_eq!(*iter.next().unwrap(), U32(2));
            assert_eq!(*iter.next().unwrap(), U32(4));
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 1);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 2);

            u32s.clear_all_modified();
        },
    );

    world.run(
        |(mut u32s, mut i16s): (ViewMut<U32, track::All>, ViewMut<I16>)| {
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

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 1);

            let mut iter = (&mut u32s, &mut i16s).iter();
            assert_eq!(iter.size_hint(), (0, Some(4)));
            let mut next = iter.next().unwrap();
            (next.0).0 += 1;
            (next.0).0 -= 1;
            assert_eq!((*next.0, *next.1), (U32(0), I16(10)));
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (U32(2), I16(12))
            );
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (U32(4), I16(14))
            );
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 1);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 2);

            u32s.clear_all_modified();
        },
    );

    world.run(
        |(mut u32s, mut i16s): (ViewMut<U32, track::All>, ViewMut<I16>)| {
            let mut iter = (&i16s, &u32s).iter();
            assert_eq!(iter.size_hint(), (0, Some(4)));
            assert_eq!(iter.next().unwrap(), (&I16(10), &U32(0)));
            assert_eq!(iter.next().unwrap(), (&I16(12), &U32(2)));
            assert_eq!(iter.next().unwrap(), (&I16(14), &U32(4)));
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 0);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 1);

            let mut iter = (&mut i16s, &mut u32s).iter();
            assert_eq!(iter.size_hint(), (0, Some(4)));
            let mut next = iter.next().unwrap();
            (next.1).0 += 1;
            (next.1).0 -= 1;
            assert_eq!((*next.0, *next.1), (I16(10), U32(0)));
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (I16(12), U32(2))
            );
            assert_eq!(
                iter.next().map(|(x, y)| (*x, *y)).unwrap(),
                (I16(14), U32(4))
            );
            assert!(iter.next().is_none());

            assert_eq!(u32s.inserted().iter().count(), 1);
            assert_eq!(u32s.modified().iter().count(), 1);
            assert_eq!(u32s.inserted_or_modified().iter().count(), 2);
        },
    );
}

#[test]
fn not_inserted() {
    let mut world = World::new();
    world.track_all::<U32>();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32, track::All>, ViewMut<I16>)>()
        .unwrap();

    entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    entities.add_entity(&mut u32s, U32(1));

    u32s.clear_all_inserted();

    let mut u32s = world.borrow::<ViewMut<U32, track::All>>().unwrap();

    entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));

    let mut iter = (!u32s.inserted()).iter();

    assert_eq!(iter.next(), Some(&U32(0)));
    assert_eq!(iter.next(), Some(&U32(1)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u32s.inserted(), &i16s).iter();

    assert_eq!(iter.next(), Some((&U32(0), &I16(10))));
    assert_eq!(iter.next(), None);
}

#[test]
fn not_modified() {
    let mut world = World::new();
    world.track_all::<U32>();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32, track::All>, ViewMut<I16>)>()
        .unwrap();

    let e0 = entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    let e1 = entities.add_entity(&mut u32s, U32(1));
    entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));

    u32s.clear_all_inserted();

    let mut u32s = world.borrow::<ViewMut<U32, track::All>>().unwrap();

    u32s[e0].0 += 100;
    u32s[e1].0 += 100;

    let mut iter = (!u32s.modified()).iter();

    assert_eq!(iter.next(), Some(&U32(2)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u32s.modified(), &i16s).iter();

    assert_eq!(iter.next(), Some((&U32(2), &I16(12))));
    assert_eq!(iter.next(), None);
}

#[test]
fn not_inserted_or_modified() {
    let mut world = World::new();
    world.track_all::<U32>();

    let (mut entities, mut u32s, mut i16s) = world
        .borrow::<(EntitiesViewMut, ViewMut<U32, track::All>, ViewMut<I16>)>()
        .unwrap();

    let e0 = entities.add_entity((&mut u32s, &mut i16s), (U32(0), I16(10)));
    entities.add_entity(&mut u32s, U32(1));

    u32s.clear_all_inserted();

    let mut u32s = world.borrow::<ViewMut<U32, track::All>>().unwrap();

    u32s[e0].0 += 100;

    entities.add_entity((&mut u32s, &mut i16s), (U32(2), I16(12)));
    entities.add_entity(&mut i16s, I16(13));

    let mut iter = (!u32s.inserted_or_modified()).iter();

    assert_eq!(iter.next(), Some(&U32(1)));
    assert_eq!(iter.next(), None);

    let mut iter = (!u32s.inserted_or_modified(), &i16s).iter();

    assert_eq!(iter.next(), None);
}
