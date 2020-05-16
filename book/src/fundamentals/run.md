# Run the World

[`run`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run) is one of three ways to modify components and entities.
It takes a single argument, a function or a closure and executes it:

```rust, noplaypen
world.run(|mut all_storages: AllStoragesViewMut| {
    // -- snip --
});
```

In this example we ask the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) for an [`AllStoragesViewMut`](https://docs.rs/shipyard/latest/shipyard/struct.AllStoragesViewMut.html), which is an exclusive view over [`AllStorages`](https://docs.rs/shipyard/latest/shipyard/struct.AllStorages.html), the storage holding all components and entities.

Storage accesses follow the same rules as Rust's borrowing: You can have as many shared accesses to a storage as you like or a single exclusive access.

When [`run`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run) executes it will borrow all requested storages at runtime, like a `RwLock`, and return a view for each.
For any storage except [`AllStorages`](https://docs.rs/shipyard/latest/shipyard/struct.AllStorages.html) they'll also need a shared access to [`AllStorages`](https://docs.rs/shipyard/latest/shipyard/struct.AllStorages.html).
Therefore we can't request both [`AllStoragesViewMut`](https://docs.rs/shipyard/latest/shipyard/struct.AllStoragesViewMut.html) and an additional storage.
It just wouldn't work.

We can work around this limitation by accessing storages through [`AllStorages`](https://docs.rs/shipyard/latest/shipyard/struct.AllStorages.html).

```rust, noplaypen
world.run(|all_storages: AllStoragesViewMut| {
    // do something with all_storages

    all_storages.run(|usizes: View<usize>| {
        // -- snip --
    });
});
```

You can find a complete list of all views in [`run`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run)'s [documentation](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run).

---

Thanks to [`run`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run) we can add entities and components to the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html).
In the next section we will see how it is done.
