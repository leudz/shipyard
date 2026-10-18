//! A small iteration regression suite built only on public APIs.
//!
//! Worlds are grouped with `World::group` and queried with the normal constructors, so the
//! planner picks each walk itself. See `benches/iteration/README.md` for what each case guards,
//! the timing boundaries and how to compare two builds.
//!
//! Every fixture is checked against its model before any timing, whether or not its cases are
//! filtered out. `--test` runs those checks plus every registered routine once.

#[path = "iteration/consumers.rs"]
mod consumers;
#[path = "iteration/fixtures.rs"]
mod fixtures;

use consumers::*;
use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, Throughput};
use fixtures::{A, B, C, Fixture};
use shipyard::{EntityId, IntoIter, View, ViewMut};
use std::hint::black_box;
use std::time::{Duration, Instant};

const SAMPLE_SIZE: usize = 30;
const WARM_UP: Duration = Duration::from_millis(250);
const MEASUREMENT: Duration = Duration::from_secs(1);

/// When set, the suite writes the cases it registered to this path once every case ran.
const MANIFEST_VAR: &str = "SHIPYARD_ITERATION_MANIFEST";

/// What a registered case measures. Comparisons only pair cases with the same id and version.
struct Case {
    id: String,
    /// Bump it when the case's fixture or consumer changes.
    version: u32,
    /// `"traversal"` or `"construction+traversal"`.
    boundary: &'static str,
    candidates: usize,
    yielded: usize,
}

/// Collects the registered cases of one Criterion group.
struct Group<'c> {
    name: &'static str,
    group: BenchmarkGroup<'c, WallTime>,
    cases: &'c mut Vec<Case>,
}

impl Group<'_> {
    /// Registers a large case. `routine` returns the time `iters` traversals took.
    fn traversal(
        &mut self,
        name: &str,
        version: u32,
        fixture: &Fixture,
        mut routine: impl FnMut(u64) -> Duration,
    ) {
        self.record(name, version, "traversal", fixture);
        self.group
            .bench_function(name, |bencher| bencher.iter_custom(&mut routine));
    }

    /// Registers a small case. Criterion times whole calls to `routine`.
    fn construction(
        &mut self,
        name: &str,
        version: u32,
        fixture: &Fixture,
        mut routine: impl FnMut() -> u32,
    ) {
        self.record(name, version, "construction+traversal", fixture);
        self.group
            .bench_function(name, |bencher| bencher.iter(&mut routine));
    }

    fn record(&mut self, name: &str, version: u32, boundary: &'static str, fixture: &Fixture) {
        self.cases.push(Case {
            id: format!("{}/{name}", self.name),
            version,
            boundary,
            candidates: fixture.candidates,
            yielded: fixture.yielded(),
        });
        self.group
            .throughput(Throughput::Elements(fixture.yielded() as u64));
    }
}

fn group<'c>(c: &'c mut Criterion, cases: &'c mut Vec<Case>, name: &'static str) -> Group<'c> {
    Group {
        name,
        group: c.benchmark_group(name),
        cases,
    }
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
    assert_eq!(hint, fixture.expected_hint(), "{case}: unexpected walk");

    let mut ids: Vec<_> = ids.collect();
    ids.sort_unstable();
    assert_eq!(ids.len(), fixture.yielded(), "{case}: wrong count");
    assert!(ids == fixture.ids(), "{case}: wrong entities");
}

fn check_sum(case: &str, got: u32, expected: u32) {
    assert_eq!(got, expected, "{case}: wrong checksum");
}

fn ordinary(c: &mut Criterion, cases: &mut Vec<Case>) {
    let mut group = group(c, cases, "ordinary");

    let dense = fixtures::dense();
    let (a, b) = dense.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("dense", &dense, hint, ids);

    check_sum("dense_next", next_pair((&a, &b).iter()), dense.pair_sum());
    group.traversal("dense_next", 1, &dense, |iters| {
        time_traversals!(iters, (&a, &b).iter(), next_pair)
    });

    let ids = next_pair_ids((&a, &b).iter().with_id());
    check_sum("dense_next_ids", ids, dense.pair_id_sum());
    group.traversal("dense_next_ids", 1, &dense, |iters| {
        time_traversals!(iters, (&a, &b).iter().with_id(), next_pair_ids)
    });

    check_sum("dense_fold", fold_pair((&a, &b).iter()), dense.pair_sum());
    group.traversal("dense_fold", 1, &dense, |iters| {
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
    group.traversal("dense_next_mut", 1, &dense_mut, |iters| {
        time_traversals!(iters, (&mut a, &b).iter(), next_pair_mut)
    });

    let sparse = fixtures::sparse();
    let (a, b) = sparse.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("sparse", &sparse, hint, ids);

    check_sum("sparse_next", next_pair((&a, &b).iter()), sparse.pair_sum());
    group.traversal("sparse_next", 1, &sparse, |iters| {
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
    group.traversal("nested_next", 1, &nested, |iters| {
        time_traversals!(iters, ((&a, &b), &c).iter(), next_nested)
    });
}

fn selection(c: &mut Criterion, cases: &mut Vec<Case>) {
    let mut group = group(c, cases, "selection");

    let small = fixtures::small_captain();
    let (a, b, c) = small.world.borrow::<(View<A>, View<B>, View<C>)>().unwrap();
    let hint = (&a, &b, &c).iter().size_hint();
    let ids = (&a, &b, &c).iter().with_id().map(|(id, _)| id);
    check_members("small_captain", &small, hint, ids);

    let sum = next_triple((&a, &b, &c).iter());
    check_sum("small_captain_next", sum, small.triple_sum());
    group.traversal("small_captain_next", 1, &small, |iters| {
        time_traversals!(iters, (&a, &b, &c).iter(), next_triple)
    });
}

fn grouped(c: &mut Criterion, cases: &mut Vec<Case>) {
    let mut group = group(c, cases, "grouped");

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
    group.traversal("aligned_fold", 1, &aligned, |iters| {
        time_traversals!(iters, (&a, &b).iter(), fold_pair)
    });

    let ids = fold_pair_ids((&a, &b).iter().with_id());
    check_sum("aligned_fold_ids", ids, aligned.pair_id_sum());
    group.traversal("aligned_fold_ids", 1, &aligned, |iters| {
        time_traversals!(iters, (&a, &b).iter().with_id(), fold_pair_ids)
    });

    let sum = for_each_pair((&a, &b).iter());
    check_sum("aligned_for_each", sum, aligned.pair_sum());
    group.traversal("aligned_for_each", 1, &aligned, |iters| {
        time_traversals!(iters, (&a, &b).iter(), for_each_pair)
    });

    // Mutation gets its own world, the read cases above keep their model.
    let aligned_mut = fixtures::aligned();
    let (mut a, b) = aligned_mut.world.borrow::<(ViewMut<A>, View<B>)>().unwrap();
    let hint = (&mut a, &b).iter().size_hint();
    let ids = (&mut a, &b).iter().with_id().map(|(id, _)| id);
    check_members("aligned_mut", &aligned_mut, hint, ids);

    fold_pair_mut((&mut a, &b).iter());
    aligned_mut.check_mutated(&a, "aligned_fold_mut");
    group.traversal("aligned_fold_mut", 1, &aligned_mut, |iters| {
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
    group.traversal("coverage_fold", 1, &coverage, |iters| {
        time_traversals!(iters, (&a, &b).iter(), fold_pair)
    });

    check_sum(
        "coverage_next",
        next_pair((&a, &b).iter()),
        coverage.pair_sum(),
    );
    group.traversal("coverage_next", 1, &coverage, |iters| {
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
    group.traversal("partial_fold", 1, &partial, |iters| {
        time_traversals!(iters, (&a, &b, &c).iter(), fold_triple)
    });
}

fn construction(c: &mut Criterion, cases: &mut Vec<Case>) {
    let mut group = group(c, cases, "construction");

    let ordinary = fixtures::small_ordinary();
    let (a, b) = ordinary.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("small_ordinary", &ordinary, hint, ids);

    let sum = construct_next_pair(&a, &b);
    check_sum("ordinary_next_103", sum, ordinary.pair_sum());
    group.construction("ordinary_next_103", 1, &ordinary, || {
        construct_next_pair(black_box(&a), black_box(&b))
    });

    let grouped = fixtures::small_grouped();
    let (a, b) = grouped.world.borrow::<(View<A>, View<B>)>().unwrap();
    let hint = (&a, &b).iter().size_hint();
    let ids = (&a, &b).iter().with_id().map(|(id, _)| id);
    check_members("small_grouped", &grouped, hint, ids);

    let sum = construct_fold_pair(&a, &b);
    check_sum("grouped_fold_103", sum, grouped.pair_sum());
    group.construction("grouped_fold_103", 1, &grouped, || {
        construct_fold_pair(black_box(&a), black_box(&b))
    });
}

/// Writes the registered cases and timing settings as JSON, for `scripts/iteration-bench.py`.
fn write_manifest(path: &std::ffi::OsStr, cases: &[Case]) {
    let cases: Vec<_> = cases
        .iter()
        .map(|case| {
            serde_json::json!({
                "id": case.id,
                "version": case.version,
                "boundary": case.boundary,
                "candidates": case.candidates,
                "yielded": case.yielded,
            })
        })
        .collect();
    let manifest = serde_json::json!({
        "sample_size": SAMPLE_SIZE,
        "warm_up_ms": WARM_UP.as_millis() as u64,
        "measurement_ms": MEASUREMENT.as_millis() as u64,
        "cases": cases,
    });

    std::fs::write(path, serde_json::to_string_pretty(&manifest).unwrap())
        .expect("couldn't write the manifest");
}

fn main() {
    let mut criterion = Criterion::default()
        .sample_size(SAMPLE_SIZE)
        .warm_up_time(WARM_UP)
        .measurement_time(MEASUREMENT)
        .without_plots()
        .configure_from_args();
    let mut cases = Vec::new();

    ordinary(&mut criterion, &mut cases);
    selection(&mut criterion, &mut cases);
    grouped(&mut criterion, &mut cases);
    construction(&mut criterion, &mut cases);

    criterion.final_summary();
    if let Some(path) = std::env::var_os(MANIFEST_VAR) {
        write_manifest(&path, &cases);
    }
}
