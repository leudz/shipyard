//! Targeted regroup benchmarks. See `regroup/README.md` for commands and timer boundaries.

#[path = "regroup/fixtures.rs"]
mod fixtures;
#[path = "regroup/grouped.rs"]
mod grouped;
#[path = "regroup/ungrouped.rs"]
mod ungrouped;

use criterion::{Criterion, criterion_group, criterion_main};

fn benchmarks(criterion: &mut Criterion) {
    grouped::spawn_ab_bulk(criterion);
    grouped::delete_ab(criterion);
    grouped::add_b(criterion);
    grouped::remove_b(criterion);
    grouped::cross_add_a(criterion);
    grouped::cross_remove_a(criterion);
    ungrouped::delete_ab(criterion);
    ungrouped::iterate_ab_full(criterion);
}

criterion_group!(regroup, benchmarks);
criterion_main!(regroup);
