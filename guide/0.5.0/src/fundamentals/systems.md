# Systems

Systems are a great way to organize code.  
A function with views as arguments is all you need.

Here's an example:

```rust, noplaypen
fn create_ints(mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>) {
    // -- snip --
}
```

We have a system, let's run it!

```rust, noplaypen
let world = World::new();

world.run(create_ints).unwrap();
```

It also works with closures.

### Passing Data to Systems

The first argument doesn't have to be a view, you can pass any data, even references.

```rust, noplaypen
fn in_acid(season: Season, positions: View<Position>, mut healths: ViewMut<Health>) {
    // -- snip --
}

let world = World::new();

world.run_with_data(in_acid, Season::Spring).unwrap();
```

We call [`run_with_data`](https://docs.rs/shipyard/0.5.0/shipyard/struct.World.html#method.run_with_data) instead of [`run`](https://docs.rs/shipyard/0.5.0/shipyard/struct.World.html#method.run) when we want to pass data to a system.

If you want to pass multiple variables, you can use a tuple.

```rust, noplaypen
fn in_acid(
    (season, precipitation): (Season, Precipitation),
    positions: View<Position>,
    mut healths: ViewMut<Health>,
) {
    // -- snip --
}

let world = World::new();

world
    .run_with_data(in_acid, (Season::Spring, Precipitation(0.1)))
    .unwrap();
```

### Workloads

A workload is a named group of systems.

```rust, noplaypen
fn create_ints(mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>) {
    // -- snip --
}

fn delete_ints(mut u32s: ViewMut<u32>) {
    // -- snip --
}

let world = World::new();

Workload::builder("Int cycle")
    .with_system(&create_ints)
    .with_system(&delete_ints)
    .add_to_world(&world)
    .unwrap();

world.run_workload("Int cycle").unwrap();
```

Workloads are stored in the [`World`](https://docs.rs/shipyard/0.5.0/shipyard/struct.World.html), ready to be run again and again.  
They don't take up much memory so even if you make a few with similar systems it's not a problem.

Workloads will run their systems first to last or at the same time when possible. We call this _outer-parallelism_, you can learn more about it in [this chapter](../going-further/parallelism.md).

#### Workload Nesting

You can also add a workload to another and build your execution logic brick by brick.

```rust, noplaypen
struct Dead<T>(core::marker::PhantomData<T>);

fn increment(mut u32s: ViewMut<u32>) {
    for mut i in (&mut u32s).iter() {
        *i += 1;
    }
}

fn flag_deleted_u32s(u32s: View<u32>, mut deads: ViewMut<Dead<u32>>) {
    for (id, i) in u32s.iter().with_id() {
        if *i > 100 {
            deads.add_component_unchecked(id, Dead(core::marker::PhantomData));
        }
    }
}

fn clear_deleted_u32s(mut all_storages: AllStoragesViewMut) {
    all_storages.delete_any::<SparseSet<Dead<u32>>>();
}

let world = World::new();

Workload::builder("Filter u32")
    .with_system(&flag_deleted_u32s)
    .with_system(&clear_deleted_u32s)
    .add_to_world(&world)
    .unwrap();

Workload::builder("Loop")
    .with_system(&increment)
    .with_workload("Filter u32")
    .add_to_world(&world)
    .unwrap();

world.run_workload("Loop").unwrap();
```

---

Congratulations, you made it to the end of the fundamentals!  
The next section will take you under the hood to learn how to get the most out of Shipyard.
