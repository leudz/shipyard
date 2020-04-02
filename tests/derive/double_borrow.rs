use shipyard::prelude::*;

#[system(Double)]
fn run(_: &u32, _: &mut u32) {}

#[system(DoubleUnique)]
fn run(_: &u32, _: Unique<&mut u32>) {}

#[system(UniqueDouble)]
fn run(_: Unique<&u32>, _: &mut u32) {}

#[system(UniqueUnique)]
fn run(_: Unique<&u32>, _: Unique<&mut u32>) {}

fn main() {}
