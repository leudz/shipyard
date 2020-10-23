use shipyard::*;

#[test]
fn on_insert() {
    static mut E: Vec<EntityId> = Vec::new();

    let world = World::new();

    let (mut entities, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<u32>)>()
        .unwrap();

    u32s.on_insert(|entity, _| unsafe {
        E.push(entity);
    });

    let e0 = entities.add_entity(&mut u32s, 0);
    let e1 = entities.add_entity(&mut u32s, 1);
    let e3 = entities.add_entity((), ());

    entities.add_component(e0, &mut u32s, 2);
    entities.add_component(e3, &mut u32s, 3);

    assert_eq!(unsafe { &*E }, &[e0, e1, e3]);
}

// #[test]
// fn on_insert_global() {
//     static mut E: Vec<EntityId> = Vec::new();

//     let world = World::new();

//     let (mut entities, mut u32s) = world
//         .try_borrow::<(EntitiesViewMut, ViewMut<u32>)>()
//         .unwrap();

//     u32s.on_insert_global(|entity, _, _| unsafe {
//         E.push(entity);
//     });

//     let e0 = entities.add_entity(&mut u32s, 0);
//     let e1 = entities.add_entity(&mut u32s, 1);

//     drop(u32s);

//     assert_eq!(unsafe { &*E }, &[e0, e1]);
// }

// #[test]
// fn on_insert_both() {
//     static mut E: Vec<EntityId> = Vec::new();

//     let world = World::new();

//     let (mut entities, mut u32s) = world
//         .try_borrow::<(EntitiesViewMut, ViewMut<u32>)>()
//         .unwrap();

//     u32s.on_insert(|entity, _| unsafe { E.push(entity) });
//     u32s.on_insert_global(|entity, _, _| unsafe {
//         E.push(entity);
//     });

//     let e0 = entities.add_entity(&mut u32s, 0);
//     let e1 = entities.add_entity(&mut u32s, 1);
//     let e3 = entities.add_entity((), ());

//     entities.add_component(e0, &mut u32s, 2);
//     entities.add_component(e3, &mut u32s, 3);

//     drop(u32s);

//     assert_eq!(unsafe { &*E }, &[e0, e1, e3, e0, e1, e3]);
// }
