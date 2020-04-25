# Create a World

[`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) is Shipyard's core data structure: it holds all data and knows how to process systems. All operations originate from one (or more) [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html).

You can create a [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) by using [`default`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.default) or [`new`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.new):

```rust, noplaypen
let world = World::default();
let world = World::new();
```

There is no need to register components. A component's storage will be created when we access it. 

Note that we didn't make `world` mutable. That is because all [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html)'s methods take a shared reference. This makes [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) easier to use across threads.

---

Now that we have a [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html), it would be nice to be able to do something with it. That's what we'll see in the next chapter!
