# Systems

Systems are a great way to organize code.  
A function with views as arguments is all you need.

Here's an example:
```rust, noplaypen
{{#include ../../../../tests/book/systems.rs:create_ints}}
```

We have a system, let's run it!

```rust, noplaypen
{{#include ../../../../tests/book/systems.rs:run}}
```

It also works with closures, all previous chapters were using systems.

### Workloads

A workload is a group of systems.

```rust, noplaypen
{{#include ../../../../tests/book/systems.rs:workload}}
```

They are stored in the [`World`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html), ready to be run again and again.  

Workloads will run their systems first to last and try to run them in parallel when possible. We call this _outer-parallelism_, you can learn more about it in [this chapter](../going-further/parallelism.md).

#### Workload Nesting

You can also add a workload to another and build your execution logic brick by brick.

```rust, noplaypen
{{#include ../../../../tests/book/systems.rs:nested_workload}}
```

---

Congratulations, you made it to the end of the fundamentals!  
The next section will explore less universal topics.
