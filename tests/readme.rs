#[test]
fn example() {
    use shipyard::{Component, IntoIter, View, ViewMut, World};

    #[derive(Component)]
    struct Health(u32);
    #[derive(Component)]
    struct Position {
        _x: f32,
        _y: f32,
    }

    fn in_acid(positions: View<Position>, mut healths: ViewMut<Health>) {
        for (_, mut health) in (&positions, &mut healths)
            .iter()
            .filter(|(pos, _)| is_in_acid(pos))
        {
            health.0 -= 1;
        }
    }

    fn is_in_acid(_: &Position) -> bool {
        // it's wet season
        true
    }

    let mut world = World::new();

    world.add_entity((Position { _x: 0.0, _y: 0.0 }, Health(1000)));

    world.run(in_acid);
}
