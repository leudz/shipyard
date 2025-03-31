# Parallelism

By late 90s - early 2000s, CPUs started to get too close to the physical limitation of transistors and manufacturers couldn't "just" make their product faster. The solution: more cores.

Nowadays almost all devices come with multiple cores, it would be a shame to use just one.

In ECS there's two big ways to split work across cores: running systems on separate threads or using a parallel iterator, we call these two methods "outer-parallelism" and "inner-parallelism," respectively.

### Outer-parallelism

We'll start by the simplest one to use. So simple that there's nothing to do, workloads handle all the work for you. We even almost used multiple threads in the [Systems chapter](../fundamentals/systems.md).

As long as the "parallel" feature is set (enabled by default) workloads will try to execute systems as much in parallel as possible. There is a set of rules that defines the "possible":
- Systems accessing [`AllStorages`](https://docs.rs/shipyard/0.8/shipyard/struct.AllStorages.html) stop all threading.
- There can't be any other access during an exclusive access, so [`ViewMut<T>`](https://docs.rs/shipyard/0.8/shipyard/struct.ViewMut.html) will block `T` threading.

When you make a workload, all systems in it will be checked and batches (groups of systems that don't conflict) will be created.  
[`add_to_world`](https://docs.rs/shipyard/0.8/shipyard/struct.Workload.html#method.add_to_world) returns information about these batches and why each system didn't get into the previous batch.

### Inner-parallelism

While parallel iterators does require us to modify our code, it's just a matter of using `par_iter` instead of `iter`.  
Don't forget to import rayon. [`par_iter`](https://docs.rs/shipyard/0.8/shipyard/trait.IntoIter.html#tymethod.par_iter) returns a [`ParallelIterator`](https://docs.rs/rayon/0.8/rayon/iter/trait.ParallelIterator.html).

Example:
```rust, noplaypen
{{#include ../../../../tests/book/parallelism.rs:import}}

{{#include ../../../../tests/book/parallelism.rs:parallelism}}
```

Don't replace all your [`iter`](https://docs.rs/shipyard/0.8/shipyard/trait.IntoIter.html#tymethod.iter) method calls just yet, however! Using a parallel iterator comes with an upfront overhead cost. It will only exceed the speed of its sequential counterpart on computations expensive enough to make up for the overhead cost in improved processing efficiency.
