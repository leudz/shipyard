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
//!   outside the timer. Regroups after changes build a fresh world for each regroup instead:
//!   `regroup*` cases make their changes outside the timer and time the regroup alone,
//!   `regroup*_cycle` cases time the changes and the regroup. Each prints a fingerprint of the
//!   layout its regroups start from, the same on two builds when they time the same layout.
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
use criterion::{BenchmarkGroup, Criterion, SamplingMode, Throughput};
use fixtures::{A, B, C, CHURN, Change, Churn, Fixture, H, Layout, Match, N, QUARTER, h_value};
use shipyard::{Component, EntityId, Get, IntoIter, View, ViewMut, World};
use std::hint::black_box;
use std::ops::Range;
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

    check_ids(case, &fixture.ids(), ids);
}

/// Asserts that `ids`, in any order, are exactly the sorted `expected`.
fn check_ids(case: &str, expected: &[EntityId], ids: impl Iterator<Item = EntityId>) {
    let mut ids: Vec<_> = ids.collect();
    ids.sort_unstable();
    assert_eq!(ids.len(), expected.len(), "{case}: wrong count");
    assert!(ids == expected, "{case}: wrong entities");
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

    // Every case below builds a fresh world for each regroup, the settings only apply to them.
    // Building one takes up to tens of milliseconds outside the timer, so we keep the iteration
    // counts low: flat sampling, with as few iterations per sample as the time allows.
    // Criterion warns that it can't complete the samples in time when a sample gets a single
    // iteration, which is expected here.
    group
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(REBUILT_WARM_UP)
        .measurement_time(REBUILT_MEASUREMENT);

    // Declaring the group on `aligned`'s entities, all of them get evaluated.
    let mut initial = fixtures::ungrouped();
    initial.world.group::<(A, B)>();
    Family::Pairs.check("regroup_initial", &initial);

    operation(&mut group, "regroup_initial", initial.yielded(), |iters| {
        time_rebuilt(
            iters,
            || fixtures::ungrouped().world,
            |world| {
                world.group::<(A, B)>();
            },
        )
    });

    for case in rebuilt_cases() {
        let mut fixture = (case.build)();
        let churn = (case.churn)(&fixture);
        fixture.churn(&churn);
        // Runs on different builds can compare these to make sure they time the same layouts.
        eprintln!(
            "lifecycle/{}: entry layout {:016x}",
            case.name,
            case.family.fingerprint(&fixture.world)
        );
        fixture.world.regroup();
        case.family.check(case.name, &fixture);
        drop(fixture);

        operation(&mut group, case.name, churn.len(), |iters| {
            time_rebuilt(
                iters,
                || {
                    let mut world = (case.build)().world;
                    churn.apply(&mut world, |_, _| {});
                    world
                },
                |world| world.regroup(),
            )
        });

        if case.cycle {
            let name = format!("{}_cycle", case.name);
            operation(&mut group, &name, churn.len(), |iters| {
                time_rebuilt(
                    iters,
                    || (case.build)().world,
                    |world| {
                        churn.apply(world, |_, _| {});
                        world.regroup();
                    },
                )
            });
        }
    }
}

/// Warm-up of the lifecycle cases rebuilding their world, measured time only.
const REBUILT_WARM_UP: Duration = Duration::from_millis(5);
/// Measured time of the lifecycle cases rebuilding their world.
const REBUILT_MEASUREMENT: Duration = Duration::from_millis(10);

/// Times `iters` runs of `routine` and returns their sum.
///
/// Each run gets a world `setup` builds before the timer starts. The world is dropped after the
/// timer stops.
fn time_rebuilt(
    iters: u64,
    mut setup: impl FnMut() -> World,
    mut routine: impl FnMut(&mut World),
) -> Duration {
    let mut total = Duration::ZERO;
    for _ in 0..iters {
        let mut world = setup();

        let start = Instant::now();
        routine(black_box(&mut world));
        total += start.elapsed();
    }
    total
}

/// A lifecycle case building a fresh world for every regroup it times.
///
/// Changes and regroups reorder components, so timing them over and over on the same world
/// would time a different layout each time, a different one on each implementation.
/// A fresh world always has the same layout: same entities, same values, same order.
struct Rebuilt {
    name: &'static str,
    family: Family,
    /// Builds the world, with the model of its queries.
    build: fn() -> Fixture,
    /// What the case does to `build`'s world before regrouping.
    churn: fn(&Fixture) -> Churn,
    /// Whether to also time the churn and the regroup together, as `<name>_cycle`.
    cycle: bool,
}

/// The lifecycle cases timing regroups after changes, see [`Rebuilt`].
///
/// Pairs in `aligned`'s world, mutated in its range:
/// - `regroup`, `regroup_tail`: B removed and given back, spread over the range or at its
///   logical end
/// - `regroup_removal`, `regroup_deletion`: B removed or entities deleted for good
/// - `regroup_members`: A or B removed, alternately, so both members are damaged
/// - `regroup_mixed`: B removed, B given back, entities deleted or A given back, in turn,
///   then new pairs
/// - `regroup_additions`: new pairs only, nothing damaged
/// - `regroup_quarter`: B removed and given back to a quarter of the pairs
///
/// `heavy`'s world, its large H never in the way of a pair query but moving with every regroup:
/// - `regroup_heavy_b`: B removed and given back, H's range isn't damaged
/// - `regroup_heavy_removal`: H removed for good
///
/// `regroup_overlap`: promotions to the union of rotated overlapping groups and demotions from it.
fn rebuilt_cases() -> Vec<Rebuilt> {
    vec![
        Rebuilt {
            name: "regroup",
            family: Family::Pairs,
            build: fixtures::aligned,
            churn: |f| Churn::each(&f.spread_matches(CHURN), reinsert_b),
            cycle: true,
        },
        Rebuilt {
            name: "regroup_tail",
            family: Family::Pairs,
            build: fixtures::aligned,
            churn: |f| Churn::each(&f.tail_matches(CHURN), reinsert_b),
            cycle: true,
        },
        Rebuilt {
            name: "regroup_removal",
            family: Family::Pairs,
            build: fixtures::aligned,
            churn: |f| Churn::each(&f.spread_matches(CHURN), |_, _| Change::RemoveB),
            cycle: true,
        },
        Rebuilt {
            name: "regroup_deletion",
            family: Family::Pairs,
            build: fixtures::aligned,
            churn: |f| Churn::each(&f.spread_matches(CHURN), |_, _| Change::Delete),
            cycle: true,
        },
        Rebuilt {
            name: "regroup_members",
            family: Family::Pairs,
            build: fixtures::aligned,
            churn: |f| {
                Churn::each(&f.spread_matches(CHURN), |k, _| {
                    if k % 2 == 0 {
                        Change::RemoveA
                    } else {
                        Change::RemoveB
                    }
                })
            },
            cycle: false,
        },
        Rebuilt {
            name: "regroup_mixed",
            family: Family::Pairs,
            build: fixtures::aligned,
            churn: |f| {
                Churn::each(&f.spread_matches(CHURN), |k, m| match k % 4 {
                    0 => Change::RemoveB,
                    1 => Change::ReinsertB(m.b),
                    2 => Change::Delete,
                    _ => Change::ReinsertA(m.a),
                })
                .adding(NEW_PAIRS)
            },
            cycle: true,
        },
        Rebuilt {
            name: "regroup_additions",
            family: Family::Pairs,
            build: fixtures::aligned,
            churn: |_| Churn::each(&[], reinsert_b).adding(NEW_PAIRS),
            cycle: false,
        },
        Rebuilt {
            name: "regroup_quarter",
            family: Family::Pairs,
            build: fixtures::aligned,
            churn: |f| Churn::each(&f.spread_matches(QUARTER), reinsert_b),
            cycle: false,
        },
        Rebuilt {
            name: "regroup_heavy_b",
            family: Family::Heavy,
            build: fixtures::heavy,
            churn: |f| Churn::each(&f.spread_matches(CHURN), reinsert_b),
            cycle: true,
        },
        Rebuilt {
            name: "regroup_heavy_removal",
            family: Family::Heavy,
            build: fixtures::heavy,
            churn: |f| Churn::each(&f.spread_matches(CHURN), |_, _| Change::RemoveH),
            cycle: false,
        },
        Rebuilt {
            name: "regroup_overlap",
            family: Family::Overlap,
            build: fixtures::overlap,
            churn: fixtures::overlap_churn,
            cycle: true,
        },
    ]
}

/// Value indices of the pairs the rebuilt cases add, after `aligned`'s.
const NEW_PAIRS: Range<u32> = N + 3..N + 3 + CHURN;

/// Gives back its B to `m`.
fn reinsert_b(_: usize, m: &Match) -> Change {
    Change::ReinsertB(m.b)
}

/// The queries checking a rebuilt case once regrouped, and the storages its fingerprint covers.
#[derive(Clone, Copy)]
enum Family {
    /// `(&A, &B)` over a single AB group.
    Pairs,
    /// `(&A, &B, &H)` over a single ABH group.
    Heavy,
    /// `(&A, &B)` and `(&A, &B, &C)` over AB, AC and their union.
    Overlap,
}

impl Family {
    /// Returns a hash of the physical order of the family's storages.
    fn fingerprint(self, world: &World) -> u64 {
        let mut hash = 0xCBF2_9CE4_8422_2325;
        hash_dense::<A>(world, &mut hash);
        hash_dense::<B>(world, &mut hash);
        match self {
            Family::Pairs => {}
            Family::Heavy => hash_dense::<H>(world, &mut hash),
            Family::Overlap => hash_dense::<C>(world, &mut hash),
        }

        hash
    }

    /// Asserts that `fixture`, regrouped, yields exactly its model through the family's queries.
    fn check(self, case: &str, fixture: &Fixture) {
        assert!(fixture.yielded() > 0, "{case}: empty workload");

        match self {
            Family::Pairs => check_aligned_pairs(case, fixture),
            Family::Heavy => check_heavy(case, fixture),
            Family::Overlap => check_overlap(case, fixture),
        }
    }
}

/// Mixes the entities of `T`'s storage into `hash` in physical order, FNV-1a style.
fn hash_dense<T: Component + Send + Sync>(world: &World, hash: &mut u64) {
    let view = world.borrow::<View<T>>().unwrap();
    for index in 0..view.len() {
        let id = view.id_at(index).unwrap();
        *hash = (*hash ^ id.inner()).wrapping_mul(0x0100_0000_01B3);
    }
}

/// Asserts that `(&A, &B)` walks a single aligned segment holding exactly the matches.
fn check_aligned_pairs(case: &str, fixture: &Fixture) {
    let (a, b) = fixture.world.borrow::<(View<A>, View<B>)>().unwrap();
    let n = fixture.yielded();

    let hint = (&a, &b).iter().size_hint();
    assert_eq!(hint, (n, Some(n)), "{case}: not a single aligned segment");
    check_ids(case, &fixture.ids(), (&a, &b).iter().ids());
    check_sum(case, fold_pair((&a, &b).iter()), fixture.pair_sum());
}

/// Asserts that `(&A, &B, &H)` walks a single aligned segment holding exactly the matches,
/// with their values.
fn check_heavy(case: &str, fixture: &Fixture) {
    let (a, b, h) = fixture
        .world
        .borrow::<(View<A>, View<B>, View<H>)>()
        .unwrap();
    let n = fixture.yielded();

    let hint = (&a, &b, &h).iter().size_hint();
    assert_eq!(hint, (n, Some(n)), "{case}: not a single aligned segment");
    check_ids(case, &fixture.ids(), (&a, &b, &h).iter().ids());
    for m in &fixture.matches {
        let (a, b, h) = (&a, &b, &h).get(m.id).unwrap();
        let values = (a.0, b.0, h.0);
        assert!(values == (m.a, m.b, h_value(m.a)), "{case}: wrong values");
    }
}

/// Asserts that `(&A, &B)` yields every match and `(&A, &B, &C)` the ones owning a C.
///
/// Members can wrap their ranges at different points, which cuts walks in several segments,
/// so the lengths are only bounded.
fn check_overlap(case: &str, fixture: &Fixture) {
    let (a, b, c) = fixture
        .world
        .borrow::<(View<A>, View<B>, View<C>)>()
        .unwrap();

    let pairs = fixture.ids();
    let hint = (&a, &b).iter().size_hint();
    assert_eq!(hint.1, Some(pairs.len()), "{case}: unexpected pair walk");
    check_ids(case, &pairs, (&a, &b).iter().ids());
    check_sum(case, fold_pair((&a, &b).iter()), fixture.pair_sum());

    let mut union: Vec<EntityId> = fixture
        .matches
        .iter()
        .filter(|m| m.c.is_some())
        .map(|m| m.id)
        .collect();
    union.sort_unstable();
    let hint = (&a, &b, &c).iter().size_hint();
    assert_eq!(hint.1, Some(union.len()), "{case}: unexpected triple walk");
    check_ids(case, &union, (&a, &b, &c).iter().ids());
    check_sum(case, fold_triple((&a, &b, &c).iter()), fixture.union_sum());
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
