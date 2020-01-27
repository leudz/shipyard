use shipyard::prelude::*;

#[system(Test)]
fn run(_: &usize, _: &mut i32, _: &Entities, _: &mut Entities, _: Unique<&usize>, _: Unique<&mut usize>, _: AllStorages, _: Entities, _: EntitiesMut, _: ThreadPool, _: &mut AllStorages, _: &ThreadPool) {}

#[system(Lifetime)]
fn run(_: &'test usize, _: &'test mut i32, _: &'test Entities, _: &'test mut Entities, _: Unique<&'test usize>, _: Unique<&'test mut usize>) {}

fn main() {}