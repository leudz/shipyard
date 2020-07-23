mod entity_id;

use shipyard::*;

#[test]
fn test() {
    let world = World::new();

    let [entity1, entity2] = world.run(
        |mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>, mut strings: ViewMut<String>| {
            strings.setup_serde(SerConfig::default());
            u32s.setup_serde(SerConfig::default());

            [
                entities.add_entity(&mut strings, "Test1212".to_string()),
                entities.add_entity((&mut u32s, &mut strings), (545, "Test741".to_string())),
            ]
        },
    );

    let mut output = Vec::new();

    world
        .serialize(
            GlobalSerConfig::default(),
            &mut serde_json::Serializer::pretty(&mut output),
        )
        .unwrap();

    println!("{}", String::from_utf8(output.clone()).unwrap());

    let world_copy = World::new_deserialized(
        GlobalDeConfig::default(),
        &mut serde_json::Deserializer::from_slice(&output),
    )
    .unwrap();

    world_copy.run(
        |entities: EntitiesView, strings: View<String>, u32s: View<u32>| {
            assert!(entities.is_alive(entity1));
            assert!(entities.is_alive(entity2));

            assert_eq!(strings.get(entity1).map(AsRef::as_ref), Ok("Test1212"));
            assert_eq!(strings.get(entity2).map(AsRef::as_ref), Ok("Test741"));
            assert_eq!(strings.len(), 2);

            assert!(u32s.get(entity1).is_err());
            assert_eq!(u32s.get(entity2), Ok(&545));
            assert_eq!(u32s.len(), 1);
        },
    );
}
