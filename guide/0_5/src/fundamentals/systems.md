# Systems

Systems are a great way to organize code.  
A function with views as arguments is all you need.

Here's an example:
```rust, noplaypen
{{#include ../../../tests/book/systems.rs:create_ints}}
```

We have a system, let's run it!

```rust, noplaypen
{{#include ../../../tests/book/systems.rs:run}}
```

It also works with closures.

### Passing Data to Systems

The first argument doesn't have to be a view, you can pass any data, even references.

```rust, noplaypen
{{#include ../../../tests/book/systems.rs:in_acid}}
```

We call [`run_with_data`](https://docs.rs/shipyard/0.5/shipyard/struct.World.html#method.run_with_data) instead of [`run`](https://docs.rs/shipyard/0.5/shipyard/struct.World.html#method.run) when we want to pass data to a system.

If you want to pass multiple variables, you can use a tuple.

```rust, noplaypen
{{#include ../../../tests/book/systems.rs:in_acid_multiple}}
```

### Workloads

A workload is a named group of systems.

```rust, noplaypen
{{#include ../../../tests/book/systems.rs:workload}}
```

Workloads are stored in the [`World`](https://docs.rs/shipyard/0.5/shipyard/struct.World.html), ready to be run again and again.  
They don't take up much memory so even if you make a few with similar systems it's not a problem.

Workloads will run their systems first to last or at the same time when possible. We call this _outer-parallelism_, you can learn more about it in [this chapter](../going-further/parallelism.md).

#### Workload Nesting

You can also add a workload to another and build your execution logic brick by brick.

```rust, noplaypen
{{#include ../../../tests/book/systems.rs:nested_workload}}
```

---

Congratulations, you made it to the end of the fundamentals!  
The next section will take you under the hood to learn how to get the most out of Shipyard.
