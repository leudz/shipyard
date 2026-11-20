//! Same-sized inputs expose captain selection; batches run inside the Rayon pool.
use criterion::{Criterion, criterion_group, criterion_main};
use rayon::prelude::*;
use shipyard::{Component, Get, IntoIter, View, ViewMut, World, track};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

struct Marker;
impl Component for Marker {
    type Tracking = track::Untracked;
}
struct Value(usize);
impl Component for Value {
    type Tracking = track::All;
}

fn consume(value: usize, work: bool) -> usize {
    if work {
        let mut hash = value;
        for _ in 0..64 {
            hash ^= hash >> 7;
            hash = hash.wrapping_mul(0x9e3779b97f4a7c15);
        }
        black_box(hash);
    }
    black_box(value)
}

fn bench(c: &mut Criterion) {
    for threads in [1, 16] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut group = c.benchmark_group(format!("planning/{threads}"));
        for name in [
            "empty",
            "tail",
            "tail_work",
            "scattered",
            "fragmented",
            "dense",
            "small_driver",
            "unfiltered",
            "or_empty",
            "or_tail",
        ] {
            const N: usize = 65_536;
            let mut world = World::new();
            let ids: Vec<_> = (0..N).map(|i| world.add_entity((Value(i),))).collect();
            for &id in ids.iter().take(if name == "small_driver" { 64 } else { N }) {
                world.add_component(id, Marker);
            }
            world.clear_all_inserted_and_modified();
            // Start writes after the global clear's timestamp.
            world.run(|_: View<Value>| {});
            let mut expected = 0;
            {
                let mut values = world.borrow::<ViewMut<Value>>().unwrap();
                for (i, &id) in ids.iter().enumerate() {
                    let changed = match name {
                        "tail" | "tail_work" | "or_tail" => i >= N - 1024,
                        "scattered" => i % 64 == 0,
                        "fragmented" => i % 128 == 0,
                        "dense" | "unfiltered" => true,
                        _ => false,
                    };
                    if changed {
                        (&mut values).get(id).unwrap().0 = i;
                        expected += i;
                    }
                }
            }
            let run = |iterations| {
                pool.install(|| {
                    let (markers, values) = world.borrow::<(View<Marker>, View<Value>)>().unwrap();
                    let query = || {
                        if name.starts_with("or_") {
                            (values.inserted() | values.modified())
                                .par_iter()
                                .map(|v| match v {
                                    shipyard::iter::OneOfTwo::One(v)
                                    | shipyard::iter::OneOfTwo::Two(v) => consume(v.0, false),
                                })
                                .sum::<usize>()
                        } else if name == "unfiltered" {
                            (&markers, &values)
                                .par_iter()
                                .map(|(_, v)| consume(v.0, false))
                                .sum::<usize>()
                        } else {
                            (&markers, values.modified())
                                .par_iter()
                                .map(|(_, v)| consume(v.0, name == "tail_work"))
                                .sum::<usize>()
                        }
                    };
                    assert_eq!(query(), expected);
                    let start = Instant::now();
                    for _ in 0..iterations {
                        black_box(query());
                    }
                    start.elapsed()
                })
            };
            group.bench_function(name, |b| b.iter_custom(run));
        }
        group.finish();
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20).warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_secs(1)).without_plots();
    targets = bench
}
criterion_main!(benches);
