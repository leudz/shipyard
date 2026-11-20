use criterion::{Criterion, criterion_group, criterion_main};
use shipyard::{Component, Get, IntoIter, View, ViewMut, World, track};
use std::{hint::black_box, time::Duration};

struct A(u64);
impl Component for A {
    type Tracking = track::Untracked;
}
struct Tracked(u64);
impl Component for Tracked {
    type Tracking = track::All;
}

fn bench(c: &mut Criterion) {
    const N: usize = 65_536;
    for grouped in [false, true] {
        let mut world = if grouped {
            World::builder().with_group::<(A, Tracked)>().build()
        } else {
            World::new()
        };
        let ids: Vec<_> = (0..N)
            .map(|i| world.add_entity((A(i as u64), Tracked(i as u64))))
            .collect();
        for i in N..3 * N {
            world.add_entity((A(i as u64),));
        }
        if grouped {
            world.regroup();
        }
        let a = world.borrow::<View<A>>().unwrap();
        let mut group = c.benchmark_group(if grouped {
            "chunks/grouped"
        } else {
            "chunks/plain"
        });
        for name in ["empty", "clustered", "scattered", "dense"] {
            world
                .borrow::<ViewMut<Tracked>>()
                .unwrap()
                .clear_all_modified();
            let mut tracked = world.borrow::<ViewMut<Tracked>>().unwrap();
            let mut expected = 0;
            for (i, &id) in ids.iter().enumerate() {
                if match name {
                    "clustered" => i < N / 64,
                    "scattered" => i % 64 == 0,
                    "dense" => true,
                    _ => false,
                } {
                    (&mut tracked).get(id).unwrap().0 = i as u64;
                    expected += 2 * i as u64;
                }
            }
            let fold = || {
                (black_box(&a), black_box(&tracked).modified())
                    .iter()
                    .fold(0, |sum, (a, t)| sum + a.0 + t.0)
            };
            assert_eq!(fold(), expected);
            group.bench_function(format!("{name}/fold"), |b| b.iter(fold));
            let next = || {
                let mut sum = 0;
                for (a, t) in (black_box(&a), black_box(&tracked).modified()).iter() {
                    sum += a.0 + t.0;
                }
                sum
            };
            assert_eq!(next(), expected);
            group.bench_function(format!("{name}/next"), |b| b.iter(next));
            if name == "dense" {
                group.bench_function("dense/first_item", |b| {
                    b.iter(|| {
                        (black_box(&a), black_box(&tracked).modified())
                            .iter()
                            .next()
                            .map(|(a, t)| a.0 + t.0)
                    })
                });
            }
        }
        let tracked = world.borrow::<View<Tracked>>().unwrap();
        group.bench_function("unfiltered/fold", |b| {
            b.iter(|| {
                (black_box(&a), black_box(&tracked))
                    .iter()
                    .fold(0, |sum, (a, t)| sum + a.0 + t.0)
            })
        });
        group.finish();
    }

    let mut world = World::new();
    let ids: Vec<_> = (0..N)
        .map(|i| world.add_entity((Tracked(i as u64),)))
        .collect();
    c.bench_function("chunks/write/get", |b| {
        b.iter(|| {
            let mut tracked = world.borrow::<ViewMut<Tracked>>().unwrap();
            for &id in &ids {
                (&mut tracked).get(black_box(id)).unwrap().0 += 1;
            }
        })
    });
    c.bench_function("chunks/write/iter", |b| {
        b.iter(|| {
            let mut tracked = world.borrow::<ViewMut<Tracked>>().unwrap();
            (&mut tracked).iter().for_each(|mut t| t.0 += 1);
        })
    });
    c.bench_function("chunks/insert", |b| {
        b.iter(|| {
            let mut world = World::new();
            for i in 0..N {
                world.add_entity((Tracked(black_box(i as u64)),));
            }
            black_box(world);
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_secs(1)).without_plots();
    targets = bench
}
criterion_main!(benches);
