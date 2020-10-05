use shipyard::*;

#[test]
fn no_pack() {
    let world = World::default();
    world
        .try_run(|mut entities: EntitiesViewMut| {
            entities.add_entity((), ());
        })
        .unwrap();
    world
        .try_run(
            |(mut entities, mut usizes, mut u32s): (
                EntitiesViewMut,
                ViewMut<usize>,
                ViewMut<u32>,
            )| {
                let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
                assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
            },
        )
        .unwrap();
}

#[test]
fn tight() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_tight_pack().unwrap();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
}

#[test]
fn loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_loose_pack().unwrap();
    let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
}

#[test]
fn tight_loose() {
    let world = World::new();
    let (mut entities, mut usizes, mut u64s, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u64>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u64s).try_tight_pack().unwrap();
    LoosePack::<(u32,)>::try_loose_pack((&mut u32s, &mut usizes, &mut u64s)).unwrap();
    let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
    assert_eq!((&usizes, &u64s, &u32s).get(entity1).unwrap(), (&0, &1, &2));
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.update_pack();
    let entity = entities.add_entity(&mut usizes, 0);
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(usizes[entity], 0);
}

#[test]
fn cleared_update() {
    let world = World::new();
    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.update_pack();
    let entity1 = entities.add_entity(&mut usizes, 1);
    usizes.try_clear_inserted_and_modified().unwrap();
    assert_eq!(usizes.inserted().iter().count(), 0);
    let entity2 = entities.add_entity(&mut usizes, 2);
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(*usizes.get(entity1).unwrap(), 1);
    assert_eq!(*usizes.get(entity2).unwrap(), 2);
}

#[test]
fn modified_update() {
    let world = World::new();
    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();
    usizes.update_pack();
    let entity1 = entities.add_entity(&mut usizes, 1);
    usizes.try_clear_inserted_and_modified().unwrap();
    usizes[entity1] = 3;
    let entity2 = entities.add_entity(&mut usizes, 2);
    assert_eq!(usizes.inserted().iter().count(), 1);
    assert_eq!(*usizes.get(entity1).unwrap(), 3);
    assert_eq!(*usizes.get(entity2).unwrap(), 2);
}

#[test]
fn not_all_tight() {
    let world = World::new();

    let (mut entities, mut u32s, mut u16s, mut f32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<u16>, ViewMut<f32>)>()
        .unwrap();

    (&mut u32s, &mut u16s).try_tight_pack().unwrap();

    entities.add_entity((&mut u32s, &mut u16s, &mut f32s), (0, 0, 0.));
    entities.add_entity((&mut f32s, &mut u16s, &mut u32s), (0., 0, 0));

    assert!((&mut u32s, &mut u16s).iter().count() > 0);
}

#[test]
fn not_all_loose() {
    let world = World::new();

    let (mut entities, mut u32s, mut u16s, mut f32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<u16>, ViewMut<f32>)>()
        .unwrap();

    (&mut u32s, &mut u16s).try_loose_pack().unwrap();

    entities.add_entity((&mut u32s, &mut u16s, &mut f32s), (0, 0, 0.));
    entities.add_entity((&mut f32s, &mut u16s, &mut u32s), (0., 0, 0));

    assert!((&mut u32s, &mut u16s).iter().count() > 0);
}

#[test]
fn builder() {
    let world = World::new();

    let _entity = world
        .try_entity_builder()
        .unwrap()
        .try_with(0usize)
        .unwrap()
        .try_with(0isize)
        .unwrap()
        .try_with(0u8)
        .unwrap()
        .try_with(0i8)
        .unwrap()
        .try_with(0u16)
        .unwrap()
        .try_with(0i16)
        .unwrap()
        .try_with(0u32)
        .unwrap()
        .try_with(0i32)
        .unwrap()
        .try_with(0u64)
        .unwrap()
        .try_with(0i64)
        .unwrap()
        .try_build()
        .unwrap();

    let _entity2 = world
        .try_borrow::<AllStoragesViewMut>()
        .unwrap()
        .entity_builder()
        .try_with(0usize)
        .unwrap()
        .try_with(0isize)
        .unwrap()
        .try_with(0u8)
        .unwrap()
        .try_with(0i8)
        .unwrap()
        .try_with(0u16)
        .unwrap()
        .try_with(0i16)
        .unwrap()
        .try_with(0u32)
        .unwrap()
        .try_with(0i32)
        .unwrap()
        .try_with(0u64)
        .unwrap()
        .try_with(0i64)
        .unwrap()
        .try_build()
        .unwrap();
}

#[cfg(all(feature = "non_send", feature = "non_sync"))]
#[test]
fn builder_non_send_sync() {
    let world = World::new();

    let _entity = world
        .try_entity_builder()
        .unwrap()
        .try_with(0usize)
        .unwrap()
        .try_with_non_send(0isize)
        .unwrap()
        .try_with_non_sync(0u8)
        .unwrap()
        .try_with_non_send_sync(0i8)
        .unwrap()
        .try_with(0u16)
        .unwrap()
        .try_with(0i16)
        .unwrap()
        .try_with(0u32)
        .unwrap()
        .try_with(0i32)
        .unwrap()
        .try_with(0u64)
        .unwrap()
        .try_with(0i64)
        .unwrap()
        .try_build()
        .unwrap();

    let _entity2 = world
        .borrow::<AllStoragesViewMut>()
        .entity_builder()
        .try_with(0usize)
        .unwrap()
        .try_with_non_send(0isize)
        .unwrap()
        .try_with_non_sync(0u8)
        .unwrap()
        .try_with_non_send_sync(0i8)
        .unwrap()
        .try_with(0u16)
        .unwrap()
        .try_with(0i16)
        .unwrap()
        .try_with(0u32)
        .unwrap()
        .try_with(0i32)
        .unwrap()
        .try_with(0u64)
        .unwrap()
        .try_with(0i64)
        .unwrap()
        .try_build()
        .unwrap();
}
