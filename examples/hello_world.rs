use shipyard::*;

#[derive(Debug)]
struct Position {
    x: f32,
    y: f32,
}

struct Velocity {
    x: f32,
    y: f32,
}

fn position_printer(positions: View<Position>) {
    for pos in positions.iter() {
        println!("position: {:?}", pos);
    }
}

fn velocity_handler(mut positions: ViewMut<Position>, velocities: View<Velocity>) {
    for (mut pos, vel) in (&mut positions, &velocities).iter() {
        pos.x += vel.x;
        pos.y += vel.y;
    }
}

fn main() {
    let world = World::new();

    world.run(
        |mut entities: EntitiesViewMut,
         mut positions: ViewMut<Position>,
         mut velocities: ViewMut<Velocity>| {
            entities.add_entity(
                (&mut positions, &mut velocities),
                (Position { x: 10., y: 10. }, Velocity { x: 10., y: 10. }),
            );
        },
    );

    loop {
        world.run(velocity_handler);
        world.run(position_printer);
    }
}
