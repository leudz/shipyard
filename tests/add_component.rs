use shipyard::*;

#[derive(PartialEq, Eq, Debug)]
struct U32(u32);
impl Component for U32 {
    type Tracking = track::Untracked;
}

#[derive(PartialEq, Eq, Debug)]
struct USIZE(usize);
impl Component for USIZE {
    type Tracking = track::Untracked;
}

#[test]
fn no_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>()
        .unwrap();

    let entity0 = entities.add_entity(&mut usizes, USIZE(0));
    let entity1 = entities.add_entity(&mut u32s, U32(1));

    let entity10 = entities.add_entity((), ());
    let entity20 = entities.add_entity((), ());
    entities.add_component(entity10, (&mut usizes, &mut u32s), (USIZE(10), U32(30)));
    entities.add_component(entity20, &mut usizes, USIZE(20));
    entities.add_component(entity20, &mut u32s, U32(50));
    assert_eq!(usizes.get(entity0).unwrap(), &USIZE(0));
    assert_eq!(u32s.get(entity1).unwrap(), &U32(1));
    assert_eq!(
        (&usizes, &u32s).get(entity10).unwrap(),
        (&USIZE(10), &U32(30))
    );
    assert_eq!(
        (&usizes, &u32s).get(entity20).unwrap(),
        (&USIZE(20), &U32(50))
    );
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), Some((&USIZE(10), &U32(30))));
    assert_eq!(iter.next(), Some((&USIZE(20), &U32(50))));
    assert_eq!(iter.next(), None);
}

#[test]
fn update() {
    let mut world = World::new();
    world.track_all::<USIZE>();
    let (mut entities, mut usizes) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE, track::All>)>()
        .unwrap();

    let entity = entities.add_entity((), ());

    entities.add_component(entity, &mut usizes, USIZE(1));

    let mut iter = usizes.inserted().iter();
    assert_eq!(iter.next(), Some(&USIZE(1)));
    assert_eq!(iter.next(), None);

    entities.add_component(entity, &mut usizes, USIZE(2));

    let mut iter = usizes.inserted().iter();
    assert_eq!(iter.next(), Some(&USIZE(2)));
    assert_eq!(iter.next(), None);

    usizes.clear_all_inserted();
    let mut usizes = world.borrow::<ViewMut<USIZE, track::All>>().unwrap();

    usizes[entity] = USIZE(3);

    entities.add_component(entity, &mut usizes, USIZE(4));

    let mut iter = usizes.modified().iter();
    assert_eq!(iter.next(), Some(&USIZE(4)));
    assert_eq!(iter.next(), None);

    usizes.clear_all_modified();

    let mut usizes = world.borrow::<ViewMut<USIZE, track::All>>().unwrap();

    entities.add_component(entity, &mut usizes, USIZE(5));

    let mut iter = usizes.modified().iter();
    assert_eq!(iter.next(), Some(&USIZE(5)));
    assert_eq!(iter.next(), None);
}

#[test]
fn no_pack_unchecked() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>()
        .unwrap();

    let entity1 = entities.add_entity((), ());
    (&mut usizes, &mut u32s).add_component_unchecked(entity1, (USIZE(0), U32(1)));
    (&mut u32s, &mut usizes).add_component_unchecked(entity1, (U32(3), USIZE(2)));
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&USIZE(2), &U32(3)));
}

#[test]
fn workload_add() {
    let mut world = World::new();

    let eid = world.add_entity(());

    world.add_workload(move || {
        (
            move |mut vm_u32: ViewMut<U32>| {
                vm_u32.add_component_unchecked(eid, U32(0));
            },
            |v_u32: View<U32, track::InsertionAndModification>| {
                assert_eq!(v_u32.inserted_or_modified().iter().count(), 1)
            },
        )
            .into_workload()
    });

    world.run_default().unwrap();
    world.run_default().unwrap();
    world.run_default().unwrap();
}

#[test]
fn workload_add_and_remove() {
    let mut world = World::new();

    let eid = world.add_entity(());

    world.add_workload(move || {
        (
            move |mut vm_u32: ViewMut<U32>| {
                vm_u32.add_component_unchecked(eid, U32(0));
            },
            |v_u32: View<U32, track::InsertionAndModification>| {
                assert_eq!(v_u32.inserted().iter().count(), 1)
            },
            move |mut vm_u32: ViewMut<U32>| {
                vm_u32.remove(eid);
            },
        )
            .into_workload()
    });

    world.run_default().unwrap();
    world.run_default().unwrap();
    world.run_default().unwrap();
}

#[test]
fn move_between_worlds() {
    let mut world1 = World::new();
    let mut world2 = World::new();

    let entity1 = world1.add_entity((USIZE(1), U32(2)));

    world2.spawn(entity1);
    world2.add_component(entity1, world1.remove::<(USIZE, U32)>(entity1));

    assert_eq!(*world2.get::<&USIZE>(entity1).unwrap(), &USIZE(1));
    assert_eq!(*world2.get::<&U32>(entity1).unwrap(), &U32(2));
}
