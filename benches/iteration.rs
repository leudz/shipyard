//! A small iteration regression suite built only on public APIs.
//!
//! Worlds are grouped with `World::group` and queried with the normal constructors, so the
//! planner picks each walk itself.
//!
//! Each Criterion group times one boundary:
//!
//! - `ordinary`, `selection`, `grouped`, `multiple`, `damage` and `parallel` time traversal.
//!   The iterator is built before the timer starts, the timer covers consuming it to
//!   exhaustion and dropping it.
//! - `lifecycle` times world operations. Restoring the world between two operations happens
//!   outside the timer.
//! - `construction+traversal` times small queries end to end: building the iterator,
//!   consuming it and dropping it.
//! - `construction+drop` times building an iterator and dropping it without advancing it.
//!
//! Every fixture is checked against its model before any timing, whether or not its cases are
//! filtered out.
//!
//! - `cargo bench --bench iteration` runs every case, a filter after `--` picks cases by id.
//! - `cargo bench --bench iteration -- --test` runs the checks plus every registered routine once.
//! - `-- --save-baseline <name>` on one build then `-- --baseline <name>` on another compares
//!   the two with Criterion's own report.

#[path = "iteration/consumers.rs"]
mod consumers;
#[path = "iteration/fixtures.rs"]
mod fixtures;

use consumers::*;
use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, Throughput};
use fixtures::{A, B, C, Fixture, Layout};
use shipyard::{EntityId, IntoIter, View, ViewMut};
use std::hint::black_box;
use std::time::{Duration, Instant};

const SAMPLE_SIZE: usize = 30;
const WARM_UP: Duration = Duration::from_millis(250);
const MEASUREMENT: Duration = Duration::from_secs(1);

/// Registers a large case. `routine` returns the time `iters` traversals took.
fn traversal(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    fixture: &Fixture,
    mut routine: impl FnMut(u64) -> Duration,
) {
    group.throughput(Throughput::Elements(fixture.yielded() as u64));
    group.bench_function(name, |bencher| bencher.iter_custom(&mut routine));
}

/// Registers a small case. Criterion times whole calls to `routine`.
fn construction_traversal(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    fixture: &Fixture,
    mut routine: impl FnMut() -> u32,
) {
    group.throughput(Throughput::Elements(fixture.yielded() as u64));
    group.bench_function(name, |bencher| bencher.iter(&mut routine));
}

/// Registers a case that builds an iterator and drops it untouched. Criterion times whole calls
/// to `routine`.
///
/// There's no throughput, no item is yielded.
fn construction_drop(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    mut routine: impl FnMut(),
) {
    group.bench_function(name, |bencher| bencher.iter(&mut routine));
}

/// Registers a world operation over `count` components, 1 for an operation without any
/// component to handle. `routine` returns the time `iters` operations took.
fn operation(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    count: usize,
    mut routine: impl FnMut(u64) -> Duration,
) {
    group.throughput(Throughput::Elements(count as u64));
    group.bench_function(name, |bencher| bencher.iter_custom(&mut routine));
}

/// Times `$iters` traversals and returns their sum.
///
/// Each traversal builds a fresh iterator with `$make` and hides it behind `black_box` before
/// the timer starts, so `$consume` can't see how it was built. The timer covers `$consume`
/// running it to exhaustion and dropping it. Mutable iterators are built, consumed and dropped
/// one at a time.
macro_rules! time_traversals {
    ($iters:expr, $make:expr, $consume:path) => {{
        let mut total = Duration::ZERO;
        for _ in 0..$iters {
            let iter = black_box($make);
            let start = Instant::now();
            let output = $consume(iter);
            total += start.elapsed();
            black_box(output);
        }
        total
    }};
}

/// Asserts that a query picked the expected walk and yields exactly the fixture's matches.
///
/// `hint` is the query's `size_hint` and `ids` the ids it yields, in any order.
fn check_members(
    case: &str,
    fixture: &Fixture,
    hint: (usize, Option<usize>),
    ids: impl Iterator<Item = EntityId>,
) {
    assert!(fixture.yielded() > 0, "{case}: empty workload");
    assert_eq!(hint, fixture.expected_hint(), "{case}: unexpected walk");

    let mut ids: Vec<_> = ids.collect();
    ids.sort_unstable();
    assert_eq!(ids.len(), fixture.yielded(), "{case}: wrong count");
    assert!(ids == fixture.ids(), "{case}: wrong entities");
}

fn check_sum(case: &str, got: u32, expected: u32) {
    assert_eq!(got, expected, "{case}: wrong checksum");
}

fn ordinary(c: &mut Criterion) {
    let mut group = c.benchmark_group("ordinary");

    let dense = fixtures::dense();
    let (a, b) = dense.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("dense", &dense, hint, ids);

    check_sum("dense_next", next_pair((&a, &b).iter()), dense.pair_sum());
    traversal(&mut group, "dense_next", &dense, |iters| {
        time_traversals!(iters, (&a, &b).iter(), next_pair)
    });

    let ids = next_pair_ids((&a, &b).iter().with_id());
    check_sum("dense_next_ids", ids, dense.pair_id_sum());
    traversal(&mut group, "dense_next_ids", &dense, |iters| {
        time_traversals!(iters, (&a, &b).iter().with_id(), next_pair_ids)
    });

    check_sum("dense_fold", fold_pair((&a, &b).iter()), dense.pair_sum());
    traversal(&mut group, "dense_fold", &dense, |iters| {
        time_traversals!(iters, (&a, &b).iter(), fold_pair)
    });

    // Mutation gets its own world, the read cases above keep their model.
    let dense_mut = fixtures::dense();
    let (mut a, b) = dense_mut.world.borrow::<(ViewMut<A>, View<B>)>().unwrap();
    let hint = (&mut a, &b).iter().size_hint();
    let ids = (&mut a, &b).iter().with_id().map(|(id, _)| id);
    check_members("dense_mut", &dense_mut, hint, ids);

    next_pair_mut((&mut a, &b).iter());
    dense_mut.check_mutated(&a, "dense_next_mut");
    traversal(&mut group, "dense_next_mut", &dense_mut, |iters| {
        time_traversals!(iters, (&mut a, &b).iter(), next_pair_mut)
    });

    let sparse = fixtures::sparse();
    let (a, b) = sparse.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("sparse", &sparse, hint, ids);

    check_sum("sparse_next", next_pair((&a, &b).iter()), sparse.pair_sum());
    traversal(&mut group, "sparse_next", &sparse, |iters| {
        time_traversals!(iters, (&a, &b).iter(), next_pair)
    });

    let nested = fixtures::nested();
    let (a, b, c) = nested
        .world
        .borrow::<(View<A>, View<B>, View<C>)>()
        .unwrap();
    let hint = ((&a, &b), &c).iter().size_hint();
    let ids = ((&a, &b), &c).iter().with_id().map(|(id, _)| id);
    check_members("nested", &nested, hint, ids);

    let sum = next_nested(((&a, &b), &c).iter());
    check_sum("nested_next", sum, nested.triple_sum());
    traversal(&mut group, "nested_next", &nested, |iters| {
        time_traversals!(iters, ((&a, &b), &c).iter(), next_nested)
    });
}

fn selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("selection");

    let small = fixtures::small_captain();
    let (a, b, c) = small.world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
    let hint = (&a, &b, &c).iter().size_hint();
    let ids = (&a, &b, &c).iter().with_id().map(|(id, _)| id);
    check_members("small_captain", &small, hint, ids);

    let sum = next_triple((&a, &b, &c).iter());
    check_sum("small_captain_next", sum, small.triple_sum());
    traversal(&mut group, "small_captain_next", &small, |iters| {
        time_traversals!(iters, (&a, &b, &c).iter(), next_triple)
    });
}

fn grouped(c: &mut Criterion) {
    let mut group = c.benchmark_group("grouped");

    let aligned = fixtures::aligned();
    let (a, b) = aligned.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("aligned", &aligned, hint, ids);

    check_sum(
        "aligned_fold",
        fold_pair((&a, &b).iter()),
        aligned.pair_sum(),
    );
    traversal(&mut group, "aligned_fold", &aligned, |iters| {
        time_traversals!(iters, (&a, &b).iter(), fold_pair)
    });

    let ids = fold_pair_ids((&a, &b).iter().with_id());
    check_sum("aligned_fold_ids", ids, aligned.pair_id_sum());
    traversal(&mut group, "aligned_fold_ids", &aligned, |iters| {
        time_traversals!(iters, (&a, &b).iter().with_id(), fold_pair_ids)
    });

    let sum = for_each_pair((&a, &b).iter());
    check_sum("aligned_for_each", sum, aligned.pair_sum());
    traversal(&mut group, "aligned_for_each", &aligned, |iters| {
        time_traversals!(iters, (&a, &b).iter(), for_each_pair)
    });

    check_sum(
        "aligned_next",
        next_pair((&a, &b).iter()),
        aligned.pair_sum(),
    );
    traversal(&mut group, "aligned_next", &aligned, |iters| {
        time_traversals!(iters, (&a, &b).iter(), next_pair)
    });

    // Mutation gets its own world, the read cases above keep their model.
    let aligned_mut = fixtures::aligned();
    let (mut a, b) = aligned_mut.world.borrow::<(ViewMut<A>, View<B>)>().unwrap();
    let hint = (&mut a, &b).iter().size_hint();
    let ids = (&mut a, &b).iter().with_id().map(|(id, _)| id);
    check_members("aligned_mut", &aligned_mut, hint, ids);

    fold_pair_mut((&mut a, &b).iter());
    aligned_mut.check_mutated(&a, "aligned_fold_mut");
    traversal(&mut group, "aligned_fold_mut", &aligned_mut, |iters| {
        time_traversals!(iters, (&mut a, &b).iter(), fold_pair_mut)
    });

    let coverage = fixtures::coverage();
    let (a, b) = coverage.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("coverage", &coverage, hint, ids);

    check_sum(
        "coverage_fold",
        fold_pair((&a, &b).iter()),
        coverage.pair_sum(),
    );
    traversal(&mut group, "coverage_fold", &coverage, |iters| {
        time_traversals!(iters, (&a, &b).iter(), fold_pair)
    });

    check_sum(
        "coverage_next",
        next_pair((&a, &b).iter()),
        coverage.pair_sum(),
    );
    traversal(&mut group, "coverage_next", &coverage, |iters| {
        time_traversals!(iters, (&a, &b).iter(), next_pair)
    });

    let partial = fixtures::partial();
    let (a, b, c) = partial
        .world
        .borrow::<(View<A>, View<B>, View<C>)>()
        .unwrap();
    let hint = (&a, &b, &c).iter().size_hint();
    let ids = (&a, &b, &c).iter().with_id().map(|(id, _)| id);
    check_members("partial", &partial, hint, ids);

    let sum = fold_triple((&a, &b, &c).iter());
    check_sum("partial_fold", sum, partial.triple_sum());
    traversal(&mut group, "partial_fold", &partial, |iters| {
        time_traversals!(iters, (&a, &b, &c).iter(), fold_triple)
    });
}

fn multiple(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple");

    let pairs = fixtures::multiple_pairs(Layout::Grouped);
    let (a, b, c) = pairs.world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("pairs", &pairs, hint, ids);
    pairs.check_wraps(&a, &b, &c, "pairs");

    check_sum("pairs_fold", fold_pair((&a, &b).iter()), pairs.pair_sum());
    traversal(&mut group, "pairs_fold", &pairs, |iters| {
        time_traversals!(iters, (&a, &b).iter(), fold_pair)
    });

    let ids = fold_pair_ids((&a, &b).iter().with_id());
    check_sum("pairs_fold_ids", ids, pairs.pair_id_sum());
    traversal(&mut group, "pairs_fold_ids", &pairs, |iters| {
        time_traversals!(iters, (&a, &b).iter().with_id(), fold_pair_ids)
    });

    // Mutation gets its own world, built the same way. The read cases above keep their model.
    let pairs_mut = fixtures::multiple_pairs(Layout::Grouped);
    let (mut a, b) = pairs_mut.world.borrow::<(ViewMut<A>, View<B>)>().unwrap();
    let hint = (&mut a, &b).iter().size_hint();
    let ids = (&mut a, &b).iter().with_id().map(|(id, _)| id);
    check_members("pairs_mut", &pairs_mut, hint, ids);

    fold_pair_mut((&mut a, &b).iter());
    pairs_mut.check_mutated(&a, "pairs_fold_mut");
    traversal(&mut group, "pairs_fold_mut", &pairs_mut, |iters| {
        time_traversals!(iters, (&mut a, &b).iter(), fold_pair_mut)
    });

    let triples = fixtures::multiple_triples(Layout::Grouped);
    let (a, b, c) = triples
        .world
        .borrow::<(View<A>, View<B>, View<C>)>()
        .unwrap();
    let hint = (&a, &b, &c).iter().size_hint();
    let ids = (&a, &b, &c).iter().with_id().map(|(id, _)| id);
    check_members("triples", &triples, hint, ids);
    triples.check_wraps(&a, &b, &c, "triples");

    let sum = fold_triple((&a, &b, &c).iter());
    check_sum("triples_fold", sum, triples.triple_sum());
    traversal(&mut group, "triples_fold", &triples, |iters| {
        time_traversals!(iters, (&a, &b, &c).iter(), fold_triple)
    });

    // The same queries on the same entities without any group.
    let pairs = fixtures::multiple_pairs(Layout::Ordinary);
    let (a, b) = pairs.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("ordinary_pairs", &pairs, hint, ids);

    let sum = fold_pair((&a, &b).iter());
    check_sum("ordinary_pairs_fold", sum, pairs.pair_sum());
    traversal(&mut group, "ordinary_pairs_fold", &pairs, |iters| {
        time_traversals!(iters, (&a, &b).iter(), fold_pair)
    });

    let triples = fixtures::multiple_triples(Layout::Ordinary);
    let (a, b, c) = triples
        .world
        .borrow::<(View<A>, View<B>, View<C>)>()
        .unwrap();
    let hint = (&a, &b, &c).iter().size_hint();
    let ids = (&a, &b, &c).iter().with_id().map(|(id, _)| id);
    check_members("ordinary_triples", &triples, hint, ids);

    let sum = fold_triple((&a, &b, &c).iter());
    check_sum("ordinary_triples_fold", sum, triples.triple_sum());
    traversal(&mut group, "ordinary_triples_fold", &triples, |iters| {
        time_traversals!(iters, (&a, &b, &c).iter(), fold_triple)
    });
}

fn damage(c: &mut Criterion) {
    let mut group = c.benchmark_group("damage");

    // The same removals and insertions, before and after regrouping, then without any group.
    let cases = [
        ("damaged_fold", fixtures::damaged()),
        ("repaired_fold", fixtures::repaired()),
        ("ordinary_fold", fixtures::churned_ordinary()),
    ];

    for (name, fixture) in &cases {
        let (a, b) = fixture.world.borrow::<(View<A>, View<B>)>().unwrap();
        let hint = (&a, &b).iter().size_hint();
        let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
        check_members(name, fixture, hint, ids);

        check_sum(name, fold_pair((&a, &b).iter()), fixture.pair_sum());
        traversal(&mut group, name, fixture, |iters| {
            time_traversals!(iters, (&a, &b).iter(), fold_pair)
        });
    }
}

#[cfg(feature = "parallel")]
fn parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel");

    for (name, layout) in [
        ("tail_fold", Layout::Grouped),
        ("ordinary_fold", Layout::Ordinary),
    ] {
        let tail = fixtures::tail(layout);
        let (a, b) = tail.world.borrow::<(View<A>, View<B>)>().unwrap();
        let hint = (&a, &b).iter().size_hint();
        let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
        check_members(name, &tail, hint, ids);

        // The tiny AB range has to come first, the rest of the work sits behind it.
        let head = &tail.matches[..fixtures::HEAD as usize];
        let first = (&a, &b).iter().ids().take(head.len());
        for id in first {
            assert!(
                head.iter().any(|m| m.id == id),
                "{name}: head not walked first"
            );
        }

        let sum = par_fold_pair((&a, &b).par_iter());
        check_sum(name, sum, tail.pair_sum());
        traversal(&mut group, name, &tail, |iters| {
            time_traversals!(iters, (&a, &b).par_iter(), par_fold_pair)
        });
    }
}

fn lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("lifecycle");

    // A full removal, restoration and regroup cycle has to leave the aligned walk as it was.
    let mut aligned = fixtures::aligned();
    let churned = aligned.churned();
    delete_b(&mut aligned.world, &churned);
    aligned.restore_b(&churned);
    aligned.world.regroup();
    {
        let (a, b) = aligned.world.borrow::<(View<A>, View<B>)>().unwrap();
        let hint = (&a, &b).iter().size_hint();
        let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
        check_members("aligned_cycle", &aligned, hint, ids);
    }

    operation(&mut group, "delete", churned.len(), |iters| {
        let mut total = Duration::ZERO;
        for _ in 0..iters {
            let start = Instant::now();
            delete_b(black_box(&mut aligned.world), &churned);
            total += start.elapsed();

            aligned.restore_b(&churned);
            aligned.world.regroup();
        }
        total
    });

    // Each regroup moves the restored B components back into the group. The removals are spread
    // over the whole range, so the first one damages almost all of it.
    operation(&mut group, "regroup", churned.len(), |iters| {
        let mut total = Duration::ZERO;
        for _ in 0..iters {
            delete_b(&mut aligned.world, &churned);
            aligned.restore_b(&churned);

            let start = Instant::now();
            black_box(&mut aligned.world).regroup();
            total += start.elapsed();
        }
        total
    });

    // With nothing to move, a regroup only goes through the storages and their groups.
    let mut clean = fixtures::aligned();
    clean.world.regroup();
    {
        let (a, b) = clean.world.borrow::<(View<A>, View<B>)>().unwrap();
        let hint = (&a, &b).iter().size_hint();
        let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
        check_members("regroup_noop", &clean, hint, ids);
    }

    operation(&mut group, "regroup_noop", 1, |iters| {
        let mut total = Duration::ZERO;
        for _ in 0..iters {
            let start = Instant::now();
            black_box(&mut clean.world).regroup();
            total += start.elapsed();
        }
        total
    });

    // The same cycle as `regroup`, with the removals at the logical end of the range:
    // regroup only moves the damaged suffix. It appends the restored pairs back at the end,
    // so every cycle damages the same suffix.
    let mut tail = fixtures::aligned();
    let tail_pairs = tail.churned_tail();
    delete_b(&mut tail.world, &tail_pairs);
    tail.restore_b(&tail_pairs);
    tail.world.regroup();
    {
        let (a, b) = tail.world.borrow::<(View<A>, View<B>)>().unwrap();
        let hint = (&a, &b).iter().size_hint();
        let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
        check_members("regroup_tail", &tail, hint, ids);
    }

    operation(&mut group, "regroup_tail", tail_pairs.len(), |iters| {
        let mut total = Duration::ZERO;
        for _ in 0..iters {
            delete_b(&mut tail.world, &tail_pairs);
            tail.restore_b(&tail_pairs);

            let start = Instant::now();
            black_box(&mut tail.world).regroup();
            total += start.elapsed();
        }
        total
    });

    let mut dense = fixtures::dense();
    let churned = dense.churned();
    operation(&mut group, "ordinary_delete", churned.len(), |iters| {
        let mut total = Duration::ZERO;
        for _ in 0..iters {
            let start = Instant::now();
            delete_b(black_box(&mut dense.world), &churned);
            total += start.elapsed();

            dense.restore_b(&churned);
        }
        total
    });
}

fn end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("construction+traversal");

    let ordinary = fixtures::small_ordinary();
    let (a, b) = ordinary.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("small_ordinary", &ordinary, hint, ids);

    let sum = construct_next_pair(&a, &b);
    check_sum("ordinary_next_103", sum, ordinary.pair_sum());
    construction_traversal(&mut group, "ordinary_next_103", &ordinary, || {
        construct_next_pair(black_box(&a), black_box(&b))
    });

    let grouped = fixtures::small_grouped();
    let (a, b) = grouped.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("small_grouped", &grouped, hint, ids);

    let sum = construct_fold_pair(&a, &b);
    check_sum("grouped_fold_103", sum, grouped.pair_sum());
    construction_traversal(&mut group, "grouped_fold_103", &grouped, || {
        construct_fold_pair(black_box(&a), black_box(&b))
    });
}

fn build_and_drop(c: &mut Criterion) {
    let mut group = c.benchmark_group("construction+drop");

    // Pairs:
    // - without any group, small and large
    // - with an ABC group that doesn't hold every match, read but unusable
    // - a single aligned segment, small and large
    // - several walks: `coverage` with its unevaluated regions, `damaged` with its damaged
    //   suffix, `pairs` with two ranges cut at wrap points
    let cases = [
        ("small_ordinary", fixtures::small_ordinary()),
        ("dense", fixtures::dense()),
        ("projection", fixtures::small_projection()),
        ("small_grouped", fixtures::small_grouped()),
        ("aligned", fixtures::aligned()),
        ("coverage", fixtures::coverage()),
        ("damaged", fixtures::damaged()),
        ("pairs", fixtures::multiple_pairs(Layout::Grouped)),
    ];

    for (name, fixture) in &cases {
        let (a, b) = fixture.world.borrow::<(View<A>, View<B>)>().unwrap();
        let hint = (&a, &b).iter().size_hint();
        let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
        check_members(name, fixture, hint, ids);

        construction_drop(&mut group, name, || {
            construct_drop_pair(black_box(&a), black_box(&b))
        });
    }

    // Flat triples: a small C captaining against a large group, then the ABC union cut at
    // wrap points.
    let cases = [
        ("small_captain", fixtures::small_captain()),
        ("triples", fixtures::multiple_triples(Layout::Grouped)),
    ];

    for (name, fixture) in &cases {
        let (a, b, c) = fixture
            .world
            .borrow::<(View<A>, View<B>, View<C>)>()
            .unwrap();
        let hint = (&a, &b, &c).iter().size_hint();
        let ids = (&a, &b, &c).iter().with_id().map(|(id, _)| id);
        check_members(name, fixture, hint, ids);

        construction_drop(&mut group, name, || {
            construct_drop_triple(black_box(&a), black_box(&b), black_box(&c))
        });
    }

    // Nested `((A, B), C)`: without any group, then driven by the nested aligned pair.
    let cases = [
        ("nested_ordinary", fixtures::nested()),
        ("nested_grouped", fixtures::partial()),
    ];

    for (name, fixture) in &cases {
        let (a, b, c) = fixture
            .world
            .borrow::<(View<A>, View<B>, View<C>)>()
            .unwrap();
        let hint = ((&a, &b), &c).iter().size_hint();
        let ids = ((&a, &b), &c).iter().with_id().map(|(id, _)| id);
        check_members(name, fixture, hint, ids);

        construction_drop(&mut group, name, || {
            construct_drop_nested(black_box(&a), black_box(&b), black_box(&c))
        });
    }

    // A filter over grouped storages keeps ordinary selection.
    let filtered = fixtures::small_filtered();
    let (a, b, c) = filtered
        .world
        .borrow::<(View<A>, View<B>, View<C>)>()
        .unwrap();
    let hint = (&a, &b, !&c).iter().size_hint();
    let ids = (&a, &b, !&c).iter().with_id().map(|(id, _)| id);
    check_members("filtered", &filtered, hint, ids);

    construction_drop(&mut group, "filtered", || {
        construct_drop_filtered(black_box(&a), black_box(&b), black_box(&c))
    });

    // Owned pairs, without and with a group. `owned_*` borrow the storages and build the
    // initial state each time, `reuse_*` build iterators from one `World::iter` holding them.
    let cases = [
        ("ordinary", fixtures::small_ordinary()),
        ("grouped", fixtures::small_grouped()),
    ];

    for (name, fixture) in &cases {
        let mut iter_ref = fixture.world.iter::<(&A, &B)>();
        let hint = iter_ref.iter().size_hint();
        let ids = iter_ref.iter().with_id().map(|(id, _)| id);
        check_members(&format!("owned_{name}"), fixture, hint, ids);

        construction_drop(&mut group, &format!("owned_{name}"), || {
            construct_drop_owned(black_box(&fixture.world))
        });
        construction_drop(&mut group, &format!("reuse_{name}"), || {
            construct_drop_reuse(black_box(&mut iter_ref))
        });
    }
}

fn main() {
    let mut criterion = Criterion::default()
        .sample_size(SAMPLE_SIZE)
        .warm_up_time(WARM_UP)
        .measurement_time(MEASUREMENT)
        .without_plots()
        .configure_from_args();

    ordinary(&mut criterion);
    selection(&mut criterion);
    grouped(&mut criterion);
    multiple(&mut criterion);
    damage(&mut criterion);
    #[cfg(feature = "parallel")]
    parallel(&mut criterion);
    lifecycle(&mut criterion);
    end_to_end(&mut criterion);
    build_and_drop(&mut criterion);

    criterion.final_summary();
}
