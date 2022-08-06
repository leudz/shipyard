use super::U32;
use shipyard::{
    AddComponent, AllStoragesViewMut, Component, EntitiesViewMut, IntoIter, IntoWithId,
    IntoWorkload, SparseSet, View, ViewMut, Workload, World,
};

#[derive(Component)]
struct Position;
#[derive(Component)]
struct Health;

#[derive(Component)]
struct Precipitation(f32);

#[allow(unused)]
// ANCHOR: create_ints
fn create_ints(mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>) {
    // -- snip --
}
// ANCHOR_END: create_ints

#[test]
#[rustfmt::skip]
fn run() {
// ANCHOR: run
let world = World::new();

world.run(create_ints);
// ANCHOR_END: run
}

#[test]
#[allow(unused)]
#[rustfmt::skip]
fn workload() {
// ANCHOR: workload
fn create_ints(mut entities: EntitiesViewMut, mut u32s: ViewMut<U32>) {
    // -- snip --
}

fn delete_ints(mut u32s: ViewMut<U32>) {
    // -- snip --
}

fn int_cycle() -> Workload {
    (create_ints, delete_ints).into_workload()
}

let world = World::new();

world.add_workload(int_cycle);

world.run_workload(int_cycle).unwrap();
// ANCHOR_END: workload
}

#[test]
#[rustfmt::skip]
fn workload_nesting() {
// ANCHOR: nested_workload
#[derive(Component)]
struct Dead<T: 'static>(core::marker::PhantomData<T>);

fn increment(mut u32s: ViewMut<U32>) {
    for mut i in (&mut u32s).iter() {
        i.0 += 1;
    }
}

fn flag_deleted_u32s(u32s: View<U32>, mut deads: ViewMut<Dead<u32>>) {
    for (id, i) in u32s.iter().with_id() {
        if i.0 > 100 {
            deads.add_component_unchecked(id, Dead(core::marker::PhantomData));
        }
    }
}

fn clear_deleted_u32s(mut all_storages: AllStoragesViewMut) {
    all_storages.delete_any::<SparseSet<Dead<u32>>>();
}

fn filter_u32() -> Workload {
    (flag_deleted_u32s, clear_deleted_u32s).into_workload()
}

fn main_loop() -> Workload {
    (increment, filter_u32).into_workload()
}

let world = World::new();

world.add_workload(main_loop);

world.run_workload(main_loop).unwrap();
// ANCHOR_END: nested_workload
}
