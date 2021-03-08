use shipyard::*;

// ANCHOR: import
use rayon::prelude::*;
// ANCHOR_END: import

#[allow(unused)]
// ANCHOR: parallelism
fn many_u32s(mut u32s: ViewMut<u32>) {
    u32s.par_iter().for_each(|i| {
        // -- snip --
    });
}
// ANCHOR_END: parallelism
