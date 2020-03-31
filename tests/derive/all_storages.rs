use shipyard::prelude::*;

#[system(All)]
fn run(_: AllStorages, _: &mut i32, _: &Entities, _: &mut Entities, _: Unique<&usize>, _: Unique<&mut usize>, _: Entities, _: EntitiesMut) {}

#[system(AllMut)]
fn run(_: &mut AllStorages, _: &mut i32, _: &Entities, _: &mut Entities, _: Unique<&usize>, _: Unique<&mut usize>, _: Entities, _: EntitiesMut) {}

fn main() {}