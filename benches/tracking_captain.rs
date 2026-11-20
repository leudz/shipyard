//! Compare tracking captain positions, including construction.
//! Validate fixtures before timing.

use criterion::{Criterion, criterion_group, criterion_main};
use shipyard::{Component, Get, IntoIter, View, ViewMut, World, track};
use std::hint::black_box;
use std::time::Duration;

struct A(u64);
impl Component for A {
    type Tracking = track::Untracked;
}
struct B(u64);
impl Component for B {
    type Tracking = track::Untracked;
}
struct Tracked(u64);
impl Component for Tracked {
    type Tracking = track::Modification;
}

fn bench(c: &mut Criterion) {
    const N: u64 = 16_384;
    for grouped in [false, true] {
        let mut world = if grouped {
            World::builder().with_group::<(A, B, Tracked)>().build()
        } else {
            World::new()
        };
        let ids: Vec<_> = (0..N)
            .map(|i| world.add_entity((A(i), B(i), Tracked(i))))
            .collect();
        // Keep tracking cheapest despite its 2x cost factor.
        for i in N..3 * N {
            world.add_entity((A(i), B(i)));
        }
        if grouped {
            world.regroup();
        }
        let (a, b) = world.borrow::<(View<A>, View<B>)>().unwrap();
        let mut group = c.benchmark_group(if grouped {
            "tracking_captain/grouped"
        } else {
            "tracking_captain/ordinary"
        });
        for (name, stride) in [("empty", 0), ("sparse", 64), ("dense", 1)] {
            world
                .borrow::<ViewMut<Tracked>>()
                .unwrap()
                .clear_all_modified();
            let mut tracked = world.borrow::<ViewMut<Tracked>>().unwrap();
            let mut expected = 0;
            if stride != 0 {
                for i in (0..N as usize).step_by(stride) {
                    let mut value = (&mut tracked).get(ids[i]).unwrap();
                    // Mark modified without changing the value.
                    value.0 = i as u64;
                    expected += 3 * i as u64;
                }
            }
            let first = || {
                (black_box(&tracked).modified(), black_box(&a), black_box(&b))
                    .iter()
                    .fold(0, |sum, (t, a, b)| sum + t.0 + a.0 + b.0)
            };
            let last = || {
                (black_box(&a), black_box(&b), black_box(&tracked).modified())
                    .iter()
                    .fold(0, |sum, (a, b, t)| sum + t.0 + a.0 + b.0)
            };
            assert_eq!(first(), expected);
            assert_eq!(last(), expected);
            group.bench_function(format!("{name}/first"), |b| b.iter(first));
            group.bench_function(format!("{name}/last"), |b| b.iter(last));
        }
        let tracked = world.borrow::<View<Tracked>>().unwrap();
        let ordinary = || {
            (black_box(&a), black_box(&b), black_box(&tracked))
                .iter()
                .fold(0, |sum, (a, b, t)| sum + a.0 + b.0 + t.0)
        };
        assert_eq!(ordinary(), 3 * N * (N - 1) / 2);
        group.bench_function("unfiltered/last", |b| b.iter(ordinary));
        group.finish();
    }
}

fn nested(c: &mut Criterion) {
    const N: u64 = 16_384;
    let mut group = c.benchmark_group("tracking_captain/nested");
    for (name, stride) in [("empty", 0), ("sparse", 64), ("dense", 1)] {
        let mut world = World::new();
        let ids: Vec<_> = (0..N).map(|i| world.add_entity((A(i), B(i)))).collect();
        let mut expected = 0;
        let mut shared = 0;
        if stride != 0 {
            for i in (0..N as usize).step_by(stride) {
                world.add_component(ids[i], Tracked(i as u64));
                expected += 3 * i as u64;
                shared += 1;
            }
        }
        // Keep the nested pair cheaper than the outer storage at every selectivity.
        for i in shared..3 * N {
            world.add_entity(Tracked(i));
        }
        let (a, b, outer) = world.borrow::<(View<A>, View<B>, View<Tracked>)>().unwrap();
        let first = || {
            ((black_box(&a), black_box(&b)), black_box(&outer))
                .iter()
                .fold(0, |sum, ((a, b), outer)| sum + a.0 + b.0 + outer.0)
        };
        let last = || {
            (black_box(&outer), (black_box(&a), black_box(&b)))
                .iter()
                .fold(0, |sum, (outer, (a, b))| sum + outer.0 + a.0 + b.0)
        };
        assert_eq!(first(), expected);
        assert_eq!(last(), expected);
        group.bench_function(format!("{name}/first"), |b| b.iter(first));
        group.bench_function(format!("{name}/last"), |b| b.iter(last));
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_secs(1))
        .without_plots();
    targets = bench, nested
}
criterion_main!(benches);
