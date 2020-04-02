use shipyard::prelude::*;

#[system(Test)]
fn run(_: &usize) where usize: Debug {}

fn main() {}
