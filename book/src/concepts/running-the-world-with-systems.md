# Running the World with Systems

There are two ways to operate on the world.

The most convenient is by using the `system` annotation and then registering it as a workload:

```
TODO: example
```

This is great because it's convenient, avoids a ton of boilerplate, and something about parallel processing (is it that each workload can run in parallel, or each system within a workload runs in parallel, or...?)

However, sometimes a world needs to be run where it has access to variables that are not registered in the ECS.

For this use case, `run` may be called explicitly:

```
TODO: example, explain params, etc.
```

(stuff that has come up - `ref mut` vs. `mut` in each place of type params, closure, iterator constructor thing, and iterator results)
