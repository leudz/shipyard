use super::Vel;
use shipyard::{
    sparse_set::SparseSet, AddComponent, AllStoragesViewMut, Component, EntitiesViewMut, IntoIter,
    IntoWorkload, View, ViewMut, Workload, World,
};

#[derive(Component)]
struct Position;
#[derive(Component)]
struct Health;

#[derive(Component)]
struct Precipitation(f32);

#[allow(unused)]
// ANCHOR: create_ints
fn create_ints(mut entities: EntitiesViewMut, mut vm_vel: ViewMut<Vel>) {
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
fn create_ints(mut entities: EntitiesViewMut, mut vm_vel: ViewMut<Vel>) {
    // -- snip --
}

fn delete_ints(mut vm_vel: ViewMut<Vel>) {
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
struct Dead<T: 'static + Send + Sync>(core::marker::PhantomData<T>);

fn increment(mut vm_vel: ViewMut<Vel>) {
    for i in (&mut vm_vel).iter() {
        i.0 += 1.0;
    }
}

fn flag_deleted_vel(v_vel: View<Vel>, mut deads: ViewMut<Dead<Vel>>) {
    for (id, i) in v_vel.iter().with_id() {
        if i.0 > 100.0 {
            deads.add_component_unchecked(id, Dead(core::marker::PhantomData));
        }
    }
}

fn clear_deleted_vel(mut all_storages: AllStoragesViewMut) {
    all_storages.delete_any::<SparseSet<Dead<Vel>>>();
}

fn filter_vel() -> Workload {
    (flag_deleted_vel, clear_deleted_vel).into_workload()
}

fn main_loop() -> Workload {
    (increment, filter_vel).into_workload()
}

let world = World::new();

world.add_workload(main_loop);

world.run_workload(main_loop).unwrap();
// ANCHOR_END: nested_workload
}
