use shipyard::*;

#[test]
fn single() {
    let world = World::new();

    world.run(|u32s: View<u32>| {
        for i in u32s.iter() {
            dbg!(i);
        }
    });
}

#[test]
fn with_id() {
    let world = World::new();

    world.run(|u32s: View<u32>| {
        for (id, i) in u32s.iter().with_id() {
            println!("{} belongs to entity {:?}", i, id);
        }
    });
}

#[test]
fn multiple() {
    let world = World::new();

    world.run(|u32s: View<u32>, usizes: View<usize>| {
        for (_i, _j) in (&u32s, &usizes).iter() {
            // -- snip --
        }
    });
}
