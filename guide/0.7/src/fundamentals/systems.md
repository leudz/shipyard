# Systems

Systems are a great way to organize code.  
A function with views as arguments is all you need.

Here's an example:
```rust, noplaypen
fn create_ints(mut entities: EntitiesViewMut, mut vm_vel: ViewMut<Vel>) {
    // -- snip --
}
```

We have a system, let's run it!

```rust, noplaypen
let world = World::new();

world.run(create_ints);
```

It also works with closures, all previous chapters were using systems.

### Workloads

A workload is a group of systems.

```rust, noplaypen
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
```

They are stored in the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html), ready to be run again and again.  

Workloads will run their systems first to last and try to run them in parallel when possible. We call this _outer-parallelism_, you can learn more about it in [this chapter](../going-further/parallelism.md).

#### Workload Nesting

You can also add a workload to another and build your execution logic brick by brick.

```rust, noplaypen
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
```

---

Congratulations, you made it to the end of the fundamentals!  
The next section will explore less universal topics.
