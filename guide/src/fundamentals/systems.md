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
world.run(create_ints).unwrap();
```

### Passing Data to Systems

The first argument doesn't have to be a view, you can pass any data to a system. You don't even have to own it. 

```rust, noplaypen
fn in_acid(season: &Season, positions: View<Position>, mut healths: ViewMut<Health>) {
    // -- snip --
}

world.run_with_data(in_acid, &season);
```
We call [`run_with_data`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run_with_data) instead of [`run`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run) when we want to pass data to a system.

If you want to pass multiple variables, you can use a tuple.

```rust, noplaypen
fn in_acid(
    (season, precipitation): (&Season, &Precipitation),
    positions: View<Position>,
    mut healths: ViewMut<Health>,
) {
    // -- snip --
}

world.run_with_data(in_acid, (&season, &precipitation)).unwrap();
```

### Workloads

A workload is a group of one or more systems that is assigned a name.  

```rust, noplaypen
fn create_ints(mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>) {
    // -- snip --
}

fn delete_ints(mut u32s: ViewMut<u32>) {
    // -- snip --
}

Workload::builder("Int cycle")
    .with_system(&create_ints)
    .with_system(&delete_ints)
    .add_to_world(&world)
    .unwrap();
```

As opposed to [`run`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run), [`add_workload`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.add_workload) won't execute any system until we ask it to.
Workloads are stored in the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html), ready to be run again and again.

```rust, noplaypen
world.run_workload("Int cycle").unwrap();
// or
world.run_default().unwrap();
```

[`run_default`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) will run the first workload added in the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run_default), or the one you choose with [`set_default_workload`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.set_default_workload).

There's a few points to keep in mind about workloads:
1. Workloads will run their systems first to last or at the same time when possible. We call this _outer-parallelism_, you can learn more about it in [this chapter](../going-further/parallelism.md).
2. A workload cannot be modified once it's defined. Think of it more as a one-time setup than something you do dynamically at runtime. Workloads don't take up much memory so even if you make a few with similar systems it's not a problem.

---

Congratulations, that's it for the fundamentals!  
