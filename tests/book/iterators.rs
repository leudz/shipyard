use super::*;

#[test]
fn single() {
    let world = World::new();

    let positions = world.borrow::<&Position>();

    (&positions).iter().for_each(|pos| {
        dbg!(pos);
    });
}

#[test]
fn with_id() {
    let world = World::new();

    let positions = world.borrow::<&Position>();

    (&positions).iter().with_id().for_each(|(id, pos)| {
        println!("Entity {:?} is at {:?}", id, pos);
    });
}

#[test]
fn multiple() {
    let world = World::new();

    let (positions, fruits) = world.borrow::<(&Position, &Fruit)>();

    (&positions, &fruits).iter().for_each(|(pos, fruit)| {
        println!("There is a {:?} at {:?}", pos, fruit);
    });
}
