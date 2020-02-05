use super::*;

struct CreateEmpty;
impl<'a> System<'a> for CreateEmpty {
    type Data = (EntitiesMut, &'a mut Empty);

    fn run(_: <Self::Data as SystemData>::View) {}
}

#[system(DestroyEmpty)]
fn run(_: &mut Entities, _: &mut Empty) {}

#[test]
fn test() {
    let world = World::new();
    world.run_system::<CreateEmpty>();
    world.add_workload::<(CreateEmpty, DestroyEmpty), _>("Empty Cycle");
    world.run_workload("Empty Cycle");
    world.run_default();
    world.run::<(EntitiesMut, &mut Empty), _, _>(|_| {});
}
