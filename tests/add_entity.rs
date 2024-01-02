use shipyard::*;

#[derive(Debug, PartialEq, Eq)]
struct U32(u32);
impl Component for U32 {}

#[test]
fn no_pack() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {}

    let world = World::new();
    world.run(|mut entities: EntitiesViewMut| {
        entities.add_entity((), ());
    });
    world.run(
        |(mut entities, mut usizes, mut u32s): (EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u32s), (USIZE(0), U32(1)));
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&USIZE(0), &U32(1)));
        },
    );
}

#[test]
fn update() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {}

    let mut world = World::new();
    world.track_all::<USIZE>();
    let (mut entities, mut usizes) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE, track::All>)>()
        .unwrap();

    let entity = entities.add_entity(&mut usizes, USIZE(0));
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes[entity], USIZE(0));
}

#[test]
fn cleared_update() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {}

    let mut world = World::new();
    world.track_all::<USIZE>();
    let (mut entities, mut usizes) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE, track::All>)>()
        .unwrap();

    let entity1 = entities.add_entity(&mut usizes, USIZE(1));
    usizes.clear_all_inserted();

    let mut usizes = world.borrow::<ViewMut<USIZE, track::All>>().unwrap();
    assert_eq!(usizes.inserted().iter().count(), 0);
    let entity2 = entities.add_entity(&mut usizes, USIZE(2));
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(*usizes.get(entity1).unwrap(), USIZE(1));
    assert_eq!(*usizes.get(entity2).unwrap(), USIZE(2));
}

#[test]
fn modified_update() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {}

    let mut world = World::new();
    world.track_all::<USIZE>();
    let (mut entities, mut usizes) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE, track::All>)>()
        .unwrap();

    let entity1 = entities.add_entity(&mut usizes, USIZE(1));
    usizes.clear_all_inserted_and_modified();

    let mut usizes = world.borrow::<ViewMut<USIZE, track::All>>().unwrap();
    usizes[entity1] = USIZE(3);
    let entity2 = entities.add_entity(&mut usizes, USIZE(2));
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(*usizes.get(entity1).unwrap(), USIZE(3));
    assert_eq!(*usizes.get(entity2).unwrap(), USIZE(2));
}
#[test]
fn bulk() {
    #[derive(Debug, PartialEq, Eq)]
    struct USIZE(usize);
    impl Component for USIZE {}

    let world = World::new();

    let (mut entities, mut usizes, mut u32s) = world
        .borrow::<(EntitiesViewMut, ViewMut<USIZE>, ViewMut<U32>)>()
        .unwrap();

    entities.bulk_add_entity((), (0..1).map(|_| {}));
    let mut new_entities = entities
        .bulk_add_entity(
            (&mut usizes, &mut u32s),
            (0..2).map(|i| (USIZE(i as usize), U32(i))),
        )
        .collect::<Vec<_>>()
        .into_iter();

    let mut iter = (&usizes, &u32s).iter().ids();
    assert_eq!(new_entities.next(), iter.next());
    assert_eq!(new_entities.next(), iter.next());
    assert_eq!(new_entities.next(), None);

    entities
        .bulk_add_entity(
            (&mut usizes, &mut u32s),
            (0..2).map(|i| (USIZE(i as usize), U32(i))),
        )
        .collect::<Vec<_>>()
        .into_iter();

    assert_eq!(usizes.len(), 4);
}

#[test]
fn bulk_unequal_length() {
    struct USIZE(usize);
    impl Component for USIZE {}

    let mut world = World::new();

    world.add_entity((U32(0),));

    let entity = world
        .bulk_add_entity((0..1).map(|_| (U32(1), USIZE(2))))
        .next()
        .unwrap();

    world.delete_entity(entity);
}

#[test]
fn workload() {
    let world = World::new();

    world.add_workload(|| {
        (
            |mut entities: EntitiesViewMut, mut vm_u32: ViewMut<U32>| {
                entities.add_entity(&mut vm_u32, U32(0));
            },
            |v_u32: View<U32, track::Insertion>| assert_eq!(v_u32.inserted().iter().count(), 1),
        )
            .into_workload()
    });

    world.run_default().unwrap();
    world.run_default().unwrap();
    world.run_default().unwrap();
}
