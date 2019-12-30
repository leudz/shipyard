# Running the World with Systems

There are two ways to operate on the world.

## System macro

The most convenient way is by using the `system` annotation and then registering it as a workload:

```rust,noplaypen
// 1. Define a system
#[system(CreateEmpty)]
pub fn run (mut entities:&mut Entities, mut empties:&mut Empty) {
    entities.add_entity(&mut empties, Empty {});
}

// 2. Add the workload
world.add_workload("Creators", CreateEmpty);

// 3. Run the workload (which will run its systems)
world.run_workload("Creators");
```

This is great because it avoids a ton of boilerplate and provides outer-parallelism without having to do anything.

Adding multiple systems to a workload is only a matter of expanding the second argument to a tuple. For example: 

```rust,noplaypen
// 1. Define another system
#[system(CreateCount)]
pub fn run (mut entities:&mut Entities, mut counts:&mut Count) {
    entities.add_entity(&mut empties, Count(0));
}

// 2. Add the workload
world.add_workload("Creators", (CreateEmpty, CreateCount));

// 3. Run the workload
world.run_workload("Creators");
```

There's a few points to keep in mind about workloads:
1. Workloads will run its systems in parallel where possible. If they can't be run in parallel, then systems run sequentially left-to-right.
2. (TODO: is this true?) A workload cannot be modified once its defined. Think of it more as a one-time-setup thing than something you do dynamically at runtime.
3. (TODO: what exactly happens?) Adding a workload with a name that already exists is an error
4. (TODO: use-case for try_add_workload()?)

So what's with the double `mut` in the system definitions (e.g. **mut** empties:&**mut** Empty)? Let's think about what we need the system to change. In this case it's for sure `entities` since we're adding a new entity, but there's more - it's actually the components too. More precisely, it's not that we're mutating the _contents_ of a specific component in this example - it's that we're adding a new component to the underlying `Storage` that contains the components. We need a mutable reference to the storage. In Shipyard terms - this means we need the system to get a `ViewMut`, e.g. a mutable _view_ into the `Storage`.

With that in mind there's actually two things we need to inform the compiler so that it can do its magic:

1. We need to tell Rust that we want a mutable reference to the variables. This is the first `mut`
2. We need to tell Shipyard that we want a `ViewMut` (as opposed to a `View`). This is the second `mut`

## Running directly

Sometimes a world needs to be run where it has access to variables that are not registered in the ECS.

For this use case, `run` may be called explicitly:

```
TODO: example, explain params, etc.
```

### System impl

It's also possible to manually define a system, without the macro, and then register that as a workload.

See [the crate docs on System](https://docs.rs/shipyard/latest/shipyard/trait.System.html) for more info.

# TODO
(stuff that has come up - `ref mut` vs. `mut` in each place of type params, closure, iterator constructor thing, and iterator results)
