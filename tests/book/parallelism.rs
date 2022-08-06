use super::Pos;
use shipyard::*;

// ANCHOR: import
use rayon::prelude::*;
// ANCHOR_END: import

#[allow(unused)]
// ANCHOR: parallelism
fn many_vm_pos(mut vm_pos: ViewMut<Pos>) {
    vm_pos.par_iter().for_each(|i| {
        // -- snip --
    });
}
// ANCHOR_END: parallelism
