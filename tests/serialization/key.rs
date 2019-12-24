use shipyard::prelude::*;

#[test]
fn key_serde() {
    let world = World::default();
    world.register::<u32>();

    //create and check a couple entities
    let (key0, _) = world.run::<(EntitiesMut, &mut u32), _, _>(|(mut entities, mut u32s)| {
        let key0 = entities.add_entity(&mut u32s, 0);
        check_roundtrip(key0, "[0,0]");

        let key1 = entities.add_entity(&mut u32s, 1);
        check_roundtrip(key1, "[1,0]");

        (key0, key1)
    });

    //delete the first entity
    world.run::<AllStorages, _, _>(|mut all_storages| {
        assert!(all_storages.delete(key0));
    });

    //add 2 more
    world.run::<(EntitiesMut, &mut u32), _, _>(|(mut entities, mut u32s)| {
        let key2 = entities.add_entity(&mut u32s, 2);
        //version was bumped
        check_roundtrip(key2, "[0,1]");

        let key3 = entities.add_entity(&mut u32s, 1);
        check_roundtrip(key3, "[2,0]");
    });
}

fn check_roundtrip(key: Key, expected: &str) {
    assert_eq!(expected, serde_json::to_string(&key).unwrap());
    let new_key: Key = serde_json::from_str(expected).unwrap();
    assert_eq!(key, new_key);
}