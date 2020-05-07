use shipyard::*;

#[test]
fn no_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    let entity1 = entities.add_entity((), ());
    entities
        .try_add_component((&mut usizes, &mut u32s), (0, 1), entity1)
        .unwrap();
    entities
        .try_add_component((&mut u32s, &mut usizes), (3, 2), entity1)
        .unwrap();
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
}

#[test]
fn tight() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_tight_pack().unwrap();
    let entity1 = entities.add_entity((), ());
    entities
        .try_add_component((&mut usizes, &mut u32s), (0, 1), entity1)
        .unwrap();
    entities
        .try_add_component((&mut usizes, &mut u32s), (3usize,), entity1)
        .unwrap();
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&3, &1));
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), Some((&3, &1)));
    assert_eq!(iter.next(), None);
}

#[test]
fn loose_add_component() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_loose_pack().unwrap();
    let entity1 = entities.add_entity((), ());
    entities
        .try_add_component((&mut usizes, &mut u32s), (0, 1), entity1)
        .unwrap();
    entities
        .try_add_component((&mut u32s, &mut usizes), (3, 2), entity1)
        .unwrap();
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
    let mut iter = (&usizes, &u32s).iter();
    assert_eq!(iter.next(), Some((&2, &3)));
    assert_eq!(iter.next(), None);
}

#[test]
fn tight_loose_add_component() {
    let world = World::new();
    let (mut entities, mut usizes, mut u64s, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u64>, ViewMut<u32>)>()
        .unwrap();

    (&mut usizes, &mut u64s).try_tight_pack().unwrap();
    LoosePack::<(u32,)>::try_loose_pack((&mut u32s, &mut usizes, &mut u64s)).unwrap();
    let entity1 = entities.add_entity((), ());
    entities
        .try_add_component((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2), entity1)
        .unwrap();
    entities
        .try_add_component((&mut u32s, &mut u64s, &mut usizes), (5, 4, 3), entity1)
        .unwrap();
    assert_eq!((&usizes, &u64s, &u32s).get(entity1).unwrap(), (&3, &4, &5));
    let mut iter = (&usizes, &u32s, &u64s).iter();
    assert_eq!(iter.next(), Some((&3, &5, &4)));
    assert_eq!(iter.next(), None);
    let mut iter = (&usizes, &u64s).iter();
    assert_eq!(iter.next(), Some((&3, &4)));
    assert_eq!(iter.next(), None);
}

#[test]
fn update() {
    let world = World::new();
    let (mut entities, mut usizes) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>)>()
        .unwrap();

    usizes.try_update_pack().unwrap();
    let entity = entities.add_entity((), ());

    entities.try_add_component(&mut usizes, 1, entity).unwrap();

    let mut iter = usizes.try_inserted().unwrap().iter();
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), None);

    entities.try_add_component(&mut usizes, 2, entity).unwrap();

    let mut iter = usizes.try_inserted().unwrap().iter();
    assert_eq!(iter.next(), Some(&2));
    assert_eq!(iter.next(), None);

    usizes.try_clear_inserted().unwrap();

    usizes[entity] = 3;

    entities.try_add_component(&mut usizes, 4, entity).unwrap();

    let mut iter = usizes.try_modified().unwrap().iter();
    assert_eq!(iter.next(), Some(&4));
    assert_eq!(iter.next(), None);

    usizes.try_clear_modified().unwrap();

    entities.try_add_component(&mut usizes, 5, entity).unwrap();

    let mut iter = usizes.try_modified().unwrap().iter();
    assert_eq!(iter.next(), Some(&5));
    assert_eq!(iter.next(), None);
}

#[test]
fn not_enough_to_tightly_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s, mut f32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>, ViewMut<f32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_tight_pack().unwrap();
    entities.add_entity(&mut u32s, 0);
    let entity = entities.add_entity((), ());
    entities
        .try_add_component((&mut f32s, &mut u32s, &mut usizes), (1., 1), entity)
        .unwrap();

    let mut iter = (&u32s).iter();
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), None);

    assert_eq!((&u32s, &f32s).get(entity), Ok((&1, &1.)));
}

#[test]
fn not_enough_to_loosely_pack() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s, mut f32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>, ViewMut<f32>)>()
        .unwrap();

    (&mut usizes, &mut u32s).try_loose_pack().unwrap();
    entities.add_entity(&mut u32s, 0);
    let entity = entities.add_entity((), ());
    entities
        .try_add_component((&mut f32s, &mut u32s, &mut usizes), (1., 1), entity)
        .unwrap();

    let mut iter = (&u32s).iter();
    assert_eq!(iter.next(), Some(&0));
    assert_eq!(iter.next(), Some(&1));
    assert_eq!(iter.next(), None);

    assert_eq!((&u32s, &f32s).get(entity), Ok((&1, &1.)));
}

#[test]
fn no_pack_unchecked() {
    let world = World::new();
    let (mut entities, mut usizes, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<usize>, ViewMut<u32>)>()
        .unwrap();

    let entity1 = entities.add_entity((), ());
    (&mut usizes, &mut u32s)
        .try_add_component_unchecked((0, 1), entity1)
        .unwrap();
    (&mut u32s, &mut usizes)
        .try_add_component_unchecked((3, 2), entity1)
        .unwrap();
    assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
}
