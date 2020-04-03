use shipyard::prelude::*;

#[system(Test1)]
fn run(_: &usize, _: &mut i32, _: &Entities, _: Unique<&u32>, _: Entities) {}

#[system(Test2)]
fn run(_: EntitiesMut, _: &mut u32) {}

#[system(Test3)]
fn run(_: &mut Entities, _: &mut u32) {}

#[system(Lifetime)]
fn run(_: &'a usize, _: &'b mut i32, _: &'c Entities, _: Unique<&'d u32>, _: Entities) {}

#[system(Modules)]
fn run(_: &mut mod1::Test, _: &mut mod2::Test) {}

fn main() {}

mod mod1 {
    pub struct Test;
}

mod mod2 {
    pub struct Test;
}
