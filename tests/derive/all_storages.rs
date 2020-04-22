use shipyard::*;

#[system(Test1)]
fn run(_: AllStorages, _: &mut i32) {}

#[system(Test2)]
fn run(_: &mut AllStorages, _: &i32) {}

#[system(Test3)]
fn run( _: &mut i32, _: AllStorages) {}

#[system(Test4)]
fn run( _: &mut i32, _: &mut AllStorages) {}

#[system(Test5)]
fn run(_: AllStorages, _: Entities) {}

#[system(Test6)]
fn run(_: AllStorages, _: EntitiesMut) {}

#[system(Test7)]
fn run(_: AllStorages, _: &Entities) {}

#[system(Test8)]
fn run(_: AllStorages, _: &mut Entities) {}

#[system(Test9)]
fn run(_: AllStorages, _: AllStorages) {}

fn main() {}
