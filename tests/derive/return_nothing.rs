// import it specifically to catch import issue with the proc macro
use shipyard::prelude::system;

#[system(Test)]
fn run(_: &usize) -> () {}

fn main() {}