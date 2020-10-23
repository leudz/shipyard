use shipyard::*;

#[test]
fn on_remove() {
    static mut E: Vec<EntityId> = Vec::new();

    let world = World::new();

    let (mut entities, mut u32s) = world
        .try_borrow::<(EntitiesViewMut, ViewMut<u32>)>()
        .unwrap();

    u32s.on_remove(|entity, _| unsafe {
        E.push(entity);
    });

    let e0 = entities.add_entity(&mut u32s, 0);
    let e1 = entities.add_entity(&mut u32s, 1);

    u32s.remove(e0);
    u32s.remove(e0);

    drop((entities, u32s));

    world.try_borrow::<AllStoragesViewMut>().unwrap().delete(e1);

    assert_eq!(unsafe { &*E }, &[e0, e1]);
}

// #[test]
// fn on_remove_global() {
//     static mut E: Vec<EntityId> = Vec::new();

//     let world = World::new();

//     let (mut entities, mut u32s) = world
//         .try_borrow::<(EntitiesViewMut, ViewMut<u32>)>()
//         .unwrap();

//     u32s.on_remove_global(|entity, _, _| unsafe {
//         E.push(entity);
//     });

//     let e0 = entities.add_entity(&mut u32s, 0);
//     let e1 = entities.add_entity(&mut u32s, 1);

//     u32s.remove(e0);
//     u32s.remove(e0);

//     drop((entities, u32s));

//     world.try_borrow::<AllStoragesViewMut>().unwrap().delete(e1);

//     assert_eq!(unsafe { &*E }, &[e0, e1]);
// }

// #[test]
// fn on_remove_both() {
//     static mut E: Vec<EntityId> = Vec::new();

//     let world = World::new();

//     let (mut entities, mut u32s) = world
//         .try_borrow::<(EntitiesViewMut, ViewMut<u32>)>()
//         .unwrap();

//     u32s.on_insert(|entity, _| unsafe { E.push(entity) });
//     u32s.on_remove_global(|entity, _, _| unsafe {
//         E.push(entity);
//     });

//     let e0 = entities.add_entity(&mut u32s, 0);
//     let e1 = entities.add_entity(&mut u32s, 1);

//     u32s.remove(e0);
//     u32s.remove(e0);

//     drop((entities, u32s));

//     world.try_borrow::<AllStoragesViewMut>().unwrap().delete(e1);

//     assert_eq!(unsafe { &*E }, &[e0, e1, e0, e1]);
// }
