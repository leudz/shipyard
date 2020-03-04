# Systems

Systems are a great way to organize code.

Here's an example:
```rust, noplaypen
struct CreateEmpty;
impl<'a> System<'a> for CreateEmpty {
    type Data = (EntitiesMut, &'a mut Empty);
    
    fn run((entities, empties): <Self::Data as SystemData>::View) { ... }
}
```

We start with an empty struct to attach the implementation on.
We then implement `System`, using its `Data` associated type to specify which storages we want to access, just like `borrow`.
Lastly, `run` will let us act on these storages. It has a single parameter: a tuple of the views of the storages we requested. You can specify the parameter's type or use `<Self::Data as SystemData>::View` as it'll always work.
Note that there is no `self` of any kind, so even if `CreateEmpty` wasn't empty, we couldn't access any of its fields.

This syntax isn't pretty, however. Now that we've seen what they look like under the hood, we can use the macro:

```rust, noplaypen
#[system(CreateEmpty)]
fn run(entities: &mut Entities, empties: &mut Empty) { ... }
```

In addition to creating the struct and implementation for us, it'll take care of lifetimes and allow us to use `&mut Entities` instead of `EntitiesMut`.

We have a system, let's run it!

```rust, noplaypen
world.run_system::<CreateEmpty>();
```

### Workloads

Running systems one by one works, but a system carries a lot of information. It would be a shame not to take advantage of it.
A workload is a group of one or more systems that is assigned a name..

```rust, noplaypen
#[system(CreateEmpty)]
fn run(entities: &mut Entities, empties: &mut Empty) { ... }

#[system(DestroyEmpty)]
fn run(entities: &mut Entities, empties: &mut Empty) { ... }

world.add_workload<(CreateEmpty, DestroyEmpty), _>("Empty Cycle");
```

As opposed to `run_system`, `add_workload` won't execute any workloads until we ask it to. Workloads are stored in the `World`, ready to be run again and again.

```rust, noplaypen
world.run_workload("Empty Cycle");
// or
world.run_default();
```

`run_default` will run the first workload added in the `World`, or the one you choose with `set_default_workload`.

There's a few points to keep in mind about workloads:
1. Workloads will run their systems left-to-right or at the same time when possible. We call this systems parallelism: outer-parallelism, you can learn more about it in [this chapter](../going-further/parallelism.md).
2. A workload cannot be modified once its defined. Think of it more as a one-time-setup thing than something you do dynamically at runtime. Workloads don't take up much memory so even if you make a few with similar systems it's not a problem.

### Anonymous system

We've seen `borrow` and systems. There's a third (and last) way to modify the `World`: `run`.

```rust, noplaypen
world.run::<(EntitiesMut, &mut Empty), _, _>(|(entities, empties)| { ... });
```

It's kind of a mix between `borrow` and systems. We request the storages access, just like `borrow`, but there's two additional generics.
The first one is for the returned value. Unlike systems, `run` can return a value.
The second one is just the full type of the closure. This closure has just one parameter: the views of the requested storages.

---

Congratulations, that's it for the fundamentals!
In the next chapter we'll use everything we learned so far to make a small game.
