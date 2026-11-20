use criterion::{Criterion, criterion_group, criterion_main};
use rayon::prelude::*;
use shipyard::{Component, IntoIter, View, ViewMut, World, track};
use std::{hint::black_box, time::Duration};

struct Pos(u64);
impl Component for Pos {
    type Tracking = track::All;
}
struct Vel(u64);
impl Component for Vel {
    type Tracking = track::Untracked;
}
struct Extra(u64);
impl Component for Extra {
    type Tracking = track::Untracked;
}

fn fixture(grouped: bool, offset: bool, late: bool) -> World {
    if late {
        let mut world = World::builder().with_group::<(Pos, Vel)>().build();
        world.bulk_add_entity((0..67).map(|_| (Pos(0),)));
        let ids: Vec<_> = world
            .bulk_add_entity((0..131_072).map(|_| (Vel(1), Extra(1))))
            .collect();
        world.regroup();
        for &id in &ids[..65_536] {
            world.add_component(id, Pos(0));
        }
        return world;
    }
    let mut world = if grouped {
        World::builder().with_group::<(Pos, Vel)>().build()
    } else {
        World::new()
    };
    if offset {
        world.bulk_add_entity((0..67).map(|_| (Pos(0),)));
        world.bulk_add_entity((0..131).map(|_| (Vel(1),)));
    }
    world.bulk_add_entity((0..65_536).map(|i| (Pos(i), Vel(1), Extra(1))));
    world.bulk_add_entity((0..65_536).map(|_| (Extra(1),)));
    if grouped {
        world.regroup();
    }
    world
}

fn bench(c: &mut Criterion) {
    for (name, grouped, offset, late) in [
        ("ordinary", false, false, false),
        ("aligned", true, false, false),
        ("partial", true, true, false),
        ("offset", true, false, true),
    ] {
        let world = fixture(grouped, offset, late);
        {
            let (pos, vel) = world.borrow::<(View<Pos>, View<Vel>)>().unwrap();
            assert_eq!((&pos, &vel).iter().count(), 65_536);
        }
        c.bench_function(&format!("mixed/{name}/next_mut_first"), |b| {
            b.iter(|| {
                let (mut pos, vel) = world.borrow::<(ViewMut<Pos>, View<Vel>)>().unwrap();
                for (mut p, v) in (&mut pos, &vel).iter() {
                    p.0 += v.0;
                }
            })
        });
        c.bench_function(&format!("mixed/{name}/next_mut_last"), |b| {
            b.iter(|| {
                let (mut pos, vel) = world.borrow::<(ViewMut<Pos>, View<Vel>)>().unwrap();
                for (v, mut p) in (&vel, &mut pos).iter() {
                    p.0 += v.0;
                }
            })
        });
        c.bench_function(&format!("mixed/{name}/fold"), |b| {
            b.iter(|| {
                let (mut pos, vel) = world.borrow::<(ViewMut<Pos>, View<Vel>)>().unwrap();
                (&mut pos, &vel).iter().for_each(|(mut p, v)| p.0 += v.0);
            })
        });
        c.bench_function(&format!("mixed/{name}/read_only"), |b| {
            b.iter(|| {
                let (mut pos, vel) = world.borrow::<(ViewMut<Pos>, View<Vel>)>().unwrap();
                for (p, v) in (&mut pos, &vel).iter() {
                    black_box(p.0 + v.0);
                }
            })
        });
        c.bench_function(&format!("mixed/{name}/take1"), |b| {
            b.iter(|| {
                let (mut pos, vel) = world.borrow::<(ViewMut<Pos>, View<Vel>)>().unwrap();
                if let Some((mut p, v)) = (&mut pos, &vel).iter().next() {
                    p.0 += v.0;
                }
            })
        });
        c.bench_function(&format!("mixed/{name}/nested"), |b| {
            b.iter(|| {
                let (mut pos, vel, extra) = world
                    .borrow::<(ViewMut<Pos>, View<Vel>, View<Extra>)>()
                    .unwrap();
                ((&mut pos, &vel), &extra)
                    .iter()
                    .for_each(|((mut p, v), e)| p.0 += v.0 + e.0);
            })
        });
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(16)
            .build()
            .unwrap();
        c.bench_function(&format!("mixed/{name}/parallel16"), |b| {
            b.iter(|| {
                pool.install(|| {
                    let (mut pos, vel) = world.borrow::<(ViewMut<Pos>, View<Vel>)>().unwrap();
                    (&mut pos, &vel)
                        .par_iter()
                        .for_each(|(mut p, v)| p.0 += v.0);
                })
            })
        });
    }
}
criterion_group! { name=benches; config=Criterion::default().sample_size(20).warm_up_time(Duration::from_millis(150)).measurement_time(Duration::from_secs(1)).without_plots(); targets=bench }
criterion_main!(benches);
