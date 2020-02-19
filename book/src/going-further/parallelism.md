# Parallelism

By late 90s - early 2000s CPUs started to get too close to the physical limitation of transistors and manufacturers couldn't "just" make their product faster, the solution: more cores.  
Nowadays almost all devices come with multiple cores, it would be a shame to use just one.  
In ECS there's two big ways to split work across cores: running systems on separate threads or using a parallel iterator, we call these two methods "outer-parallelism" and "inner-parallelism" respectively.

### Outer-parallelism

We'll start by the most simple one to use. So simple that there's nothing to do, workloads handle all the work for you. We even almost used multiple threads in the [Systems chapter](../fundamentals/systems.md).  
As long as the "parallel" feature is set, workloads will try to execute systems as much in parallel as possible. There is a set of rules that defines the "possible":
- systems accessing `AllStorages` stop all multithreading (this is a limit of the current implementation and will be relaxed a little)
- there can't be any other access during an exclusive access, so `&mut T` will block `T` threading

When you make a workload all systems in it will be checked and batches (group of systems that don't conflict) will be created.

There's just a problem with this approach, what happens when I want to force two non conflicting systems to run one after the other? `FakeBorrow` is here just for that, it'll mimic a system accessing the storage exclusively without actually doing it.

### Inner-parallelism

While parallel iterators require us to modify our code it's just a matter or adding `par_` to `iter`.
Don't forget to import rayon, `par_iter` will return a `ParallelIterator`.

Example:
```rust, noplaypen
use rayon::prelude::*;

#[system(ManyUsize)]
fn run(usizes: &mut usize) {
    usizes.par_iter().for_each(|i| {
        // -- snip --
    });
}
```

Don't replace all your `iter` just yet however. Using a parallel iterator comes with an upfront cost that have to be compensate before reaching the same speed as its sequential counterpart. That's why they're useful when you have a lot of values.

---

In the next chapter we'll see how packs leverage `SparseSet` to add functionalities and/or gain performance.
