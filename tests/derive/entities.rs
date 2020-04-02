use shipyard::prelude::*;

#[system(Test1)]
fn run(_: Entities, _: EntitiesMut) {}

#[system(Test2)]
fn run(_: Entities, _: &mut Entities) {}

#[system(Test3)]
fn run(_: &Entities, _: EntitiesMut) {}

#[system(Test4)]
fn run(_: &Entities, _: &mut Entities) {}

fn main() {}
