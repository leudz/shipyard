//! The runtime cost of automatic workload regroups, built only on public APIs.
//!
//! Worlds, workload registration and checks all happen before any timing.
//!
//! - `clean`: Criterion's normal loop times `World::regroup` and `World::regroup_set::<A>` on a
//!   small, clean world, with the `(A, B)` group alone or next to an unrelated `(C, D)` group.
//! - `workload`: Criterion's normal loop times one run of a small workload, `World::run_workload`
//!   for the ones added to a world, `ScheduledWorkload::run_with_world` for the standalone ones.
//!   Each case comes in up to three variants, the same workload from the same factory each time:
//!   - `registered`: added to a grouped world, with its automatic regroups
//!   - `standalone`: built with `Workload::build`, run on an identical grouped world, no regroup
//!   - `group_free`: added to an identical world without any group
//! - `structural`: one run of a workload removing B from `CHURN` pairs and giving it back, then
//!   folding every pair, on a fresh `aligned` world for each run. Building it, and adding the
//!   workload for `registered`, happens outside the timer, dropping it after the timer stops.
//!   `standalone` does the same work with standalone workloads: an explicit `regroup_set` for the
//!   entry regroup, the writer, another `regroup_set` before the reader, then the reader.
//! - `probe`: one run of a registered workload whose first batch shares two entry regroups, with
//!   at least one of the regroups left after the calling thread's step having work to do, see
//!   `probe.rs`. A persistent world is damaged again before each run, outside the timer, each run
//!   is timed on its own and checked after the timer stops.
//! - `pending`: Criterion's normal loop times one run of a registered workload leaving twelve
//!   grouped storages pending at once, see `pending.rs`.
//!
//! Every world is checked, and every workload run and checked, before any timing, whether or not
//! its cases are filtered out.
//!
//! - `cargo bench --bench workload_regroup` runs every case, a filter after `--` picks cases by id.
//! - `cargo bench --bench workload_regroup -- --test` runs the checks plus every routine once.

#[path = "workload_regroup/fixtures.rs"]
mod fixtures;
// We only need its components, values and the `aligned` fixture with its model.
#[allow(dead_code)]
#[path = "iteration/fixtures.rs"]
mod iteration_fixtures;
#[path = "workload_regroup/pending.rs"]
mod pending;
#[path = "workload_regroup/probe.rs"]
mod probe;
#[path = "workload_regroup/structural.rs"]
mod structural;

use criterion::{Criterion, SamplingMode};
use fixtures::{CASES, Groups};
use iteration_fixtures::A;
use shipyard::World;
use std::hint::black_box;
use std::time::{Duration, Instant};
use structural::{DAMAGES, Structural};

const SAMPLE_SIZE: usize = 30;
const WARM_UP: Duration = Duration::from_millis(250);
const MEASUREMENT: Duration = Duration::from_secs(1);

/// Warm-up of the cases rebuilding their world, measured time only.
const REBUILT_WARM_UP: Duration = Duration::from_millis(5);
/// Measured time of the cases rebuilding their world.
const REBUILT_MEASUREMENT: Duration = Duration::from_millis(10);

fn clean(c: &mut Criterion) {
    let mut group = c.benchmark_group("clean");

    for (suffix, groups) in [("", Groups::Ab), ("_two_sets", Groups::AbCd)] {
        let mut small = fixtures::small(groups);
        small.check_members(&format!("clean{suffix}"));

        let name = format!("regroup{suffix}");
        group.bench_function(&name, |bencher| {
            bencher.iter(|| black_box(&mut small.world).regroup())
        });
        small.check_members(&name);

        let name = format!("regroup_set{suffix}");
        group.bench_function(&name, |bencher| {
            bencher.iter(|| black_box(&small.world).regroup_set::<A>().unwrap())
        });
        small.check_members(&name);
    }
}

fn workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("workload");

    for case in CASES {
        for &variant in case.variants() {
            let label = format!("{}/{}", case.name(), variant.name());
            let prepared = fixtures::prepare(case, variant);
            fixtures::check_runs(&prepared, &label);

            let world = &prepared.small.world;
            match &prepared.standalone {
                Some(scheduled) => group.bench_function(&label, |bencher| {
                    bencher.iter(|| scheduled.run_with_world(black_box(world)).unwrap())
                }),
                None => group.bench_function(&label, |bencher| {
                    bencher.iter(|| black_box(world).run_workload(case.name()).unwrap())
                }),
            };

            fixtures::check_runs(&prepared, &label);
        }
    }
}

fn structural(c: &mut Criterion) {
    let mut group = c.benchmark_group("structural");

    // Every case builds a fresh world for each run, taking milliseconds outside the timer, so we
    // keep the iteration counts low like the iteration benchmark's lifecycle cases: flat sampling,
    // with as few iterations per sample as the time allows. Criterion warns that it can't
    // complete the samples in time when a sample gets a single iteration, which is expected.
    group
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(REBUILT_WARM_UP)
        .measurement_time(REBUILT_MEASUREMENT);

    for damage in DAMAGES {
        let case = Structural::new(damage);
        case.check();

        let name = format!("{}/registered", damage.name());
        group.bench_function(name, |bencher| {
            bencher.iter_custom(|iters| {
                time_rebuilt(
                    iters,
                    || case.registered_world(),
                    |world| case.run_registered(world),
                )
            })
        });

        let name = format!("{}/standalone", damage.name());
        group.bench_function(name, |bencher| {
            bencher.iter_custom(|iters| {
                time_rebuilt(iters, || case.world(), |world| case.run_standalone(world))
            })
        });
    }
}

fn pending(c: &mut Criterion) {
    let mut group = c.benchmark_group("pending");

    let pending = pending::Pending::new();
    pending.check();

    group.bench_function("twelve", |bencher| {
        bencher.iter(|| black_box(&pending).run())
    });

    pending.check();
}

fn probe(c: &mut Criterion) {
    let mut group = c.benchmark_group("probe");

    for probe in probe::PROBES {
        let mut prepared = probe::Prepared::new(probe);
        for _ in 0..3 {
            prepared.damage();
            prepared.run();
            prepared.check_run();
        }
        // Runs on different builds can compare these to make sure they repair the same layout.
        eprintln!(
            "probe/{}: damaged layout {:016x}",
            probe.name(),
            prepared.damaged_layout()
        );

        group.bench_function(probe.name(), |bencher| {
            bencher.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    prepared.damage();

                    let start = Instant::now();
                    prepared.run();
                    total += start.elapsed();

                    prepared.check_run();
                }
                total
            })
        });
    }
}

/// Times `iters` runs of `routine` and returns their sum.
///
/// Each run gets a world `setup` builds before the timer starts. The world is dropped after the
/// timer stops.
fn time_rebuilt(
    iters: u64,
    mut setup: impl FnMut() -> World,
    mut routine: impl FnMut(&World),
) -> Duration {
    let mut total = Duration::ZERO;
    for _ in 0..iters {
        let world = setup();

        let start = Instant::now();
        routine(black_box(&world));
        total += start.elapsed();
    }
    total
}

fn main() {
    let mut criterion = Criterion::default()
        .sample_size(SAMPLE_SIZE)
        .warm_up_time(WARM_UP)
        .measurement_time(MEASUREMENT)
        .without_plots()
        .configure_from_args();

    clean(&mut criterion);
    workload(&mut criterion);
    structural(&mut criterion);
    probe(&mut criterion);
    pending(&mut criterion);

    criterion.final_summary();
}
