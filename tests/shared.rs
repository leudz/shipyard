use shipyard::*;
/*
#[test]
fn get() {
    let world = World::new();
    world.run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
        let entity = entities.add_entity(&mut u32s, 0);
        let entity2 = entities.add_entity((), ());
        u32s.share(entity, entity2);
        assert_eq!(u32s.get(entity2), &0);
        u32s.unshare(entity2);
        assert_eq!(u32s.get(entity), &0);
        assert!(u32s.try_get(entity2).is_err());
    });
}
*/
