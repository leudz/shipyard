use criterion::{Criterion, criterion_group, criterion_main};
use rayon::prelude::*;
use shipyard::{Component, Get, IntoIter, ViewMut, World, track};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
struct Tracked(u64);
impl Component for Tracked {
    type Tracking = track::All;
}
fn bench_view(
    c: &mut Criterion,
    world: &World,
    name: &str,
    mut f: impl FnMut(&mut ViewMut<Tracked>),
) {
    c.bench_function(&format!("metadata/{name}"), |b| {
        b.iter(|| f(&mut world.borrow::<ViewMut<Tracked>>().unwrap()))
    });
}
fn bench(c: &mut Criterion) {
    const N: usize = 65_536;
    let mut world = World::new();
    let ids: Vec<_> = (0..N)
        .map(|i| world.add_entity((Tracked(i as u64),)))
        .collect();
    bench_view(c, &world, "get", |v| {
        for &id in &ids {
            (&mut *v).get(black_box(id)).unwrap().0 += 1;
        }
    });
    bench_view(c, &world, "iter", |v| v.iter().for_each(|mut t| t.0 += 1));
    bench_view(c, &world, "repeat", |v| {
        v.iter().for_each(|mut t| {
            for _ in 0..8 {
                black_box(&mut t).0 += 1;
            }
        })
    });
    bench_view(c, &world, "sparse", |v| {
        for &id in ids.iter().step_by(64) {
            (&mut *v).get(black_box(id)).unwrap().0 += 1;
        }
    });
    bench_view(c, &world, "read_only", |v| {
        v.iter().for_each(|t| {
            black_box(t.0);
        })
    });
    bench_view(c, &world, "first", |v| {
        v.iter().enumerate().for_each(|(index, mut t)| {
            if index % 64 == 0 {
                t.0 += 1;
            }
        })
    });
    bench_view(c, &world, "last", |v| {
        v.iter().enumerate().for_each(|(index, mut t)| {
            if index % 64 == 63 {
                t.0 += 1;
            }
        })
    });
    bench_view(c, &world, "for_loop", |v| {
        for mut t in v.iter() {
            t.0 += 1;
        }
    });
    bench_view(c, &world, "next_one", |v| {
        if let Some(mut t) = v.iter().next() {
            t.0 += 1;
        }
    });
    bench_view(c, &world, "next_64", |v| {
        for mut t in v.iter().take(64) {
            t.0 += 1;
        }
    });
    bench_view(c, &world, "next_read", |v| {
        for t in v.iter() {
            black_box(t.0);
        }
    });
    bench_view(c, &world, "with_id", |v| {
        v.iter()
            .with_id()
            .for_each(|(id, mut t)| t.0 += id.index() + 1)
    });
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(16)
        .build()
        .unwrap();
    c.bench_function("metadata/parallel16", |b| {
        b.iter_custom(|iterations| {
            pool.install(|| {
                let start = Instant::now();
                for _ in 0..iterations {
                    let mut v = world.borrow::<ViewMut<Tracked>>().unwrap();
                    (&mut v).par_iter().for_each(|mut t| t.0 += 1);
                }
                start.elapsed()
            })
        })
    });
}
criterion_group! { name=benches; config=Criterion::default().sample_size(20).warm_up_time(Duration::from_millis(200)).measurement_time(Duration::from_secs(1)).without_plots(); targets=bench }
criterion_main!(benches);
