# Systems

Systems are a great way to organize code. A function taking only views as arguments is all you need.

Here's an example:
```rust, noplaypen
fn create_ints(mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>) {
    // -- snip --
}
```

We have a system, let's run it!

```rust, noplaypen
world.run(create_ints);
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

world
    .add_workload("Int cycle")
    .with_system((|world: &World| world.try_run(create_ints), create_ints))
    .with_system(system!(delete_ints))
    .build();
```

The repetition in [`with_system`](https://docs.rs/shipyard/latest/shipyard/struct.WorkloadBuilder.html#method.with_system)'s argument is needed for now. It's important to have the same function twice, the workload could fail to run very time if it isn't the case.  
The [`system!`](https://docs.rs/shipyard/latest/shipyard/macro.system.html) macro will take care of the repetition for us and prevent human error.

As opposed to [`run`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run), [`add_workload`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.add_workload) won't execute any system until we ask it to. Workloads are stored in the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html), ready to be run again and again.

```rust, noplaypen
world.run_workload("Int cycle");
// or
world.run_default();
```

[`run_default`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) will run the first workload added in the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run_default), or the one you choose with [`set_default_workload`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.set_default_workload).

There's a few points to keep in mind about workloads:
1. Workloads will run their systems first to last or at the same time when possible. We call this systems parallelism: outer-parallelism, you can learn more about it in [this chapter](../going-further/parallelism.md).
2. A workload cannot be modified once it's defined. Think of it more as a one-time setup than something you do dynamically at runtime. Workloads don't take up much memory so even if you make a few with similar systems it's not a problem.

---

Congratulations, that's it for the fundamentals!  
In the next chapter we'll use everything we learned so far to make a small game.
