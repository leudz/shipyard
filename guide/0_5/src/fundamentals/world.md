# World

[`World`](https://docs.rs/shipyard/0.5/shipyard/struct.World.html) is Shipyard's core data structure: It holds all data and knows how to process systems. All operations originate from one (or more) [`World`](https://docs.rs/shipyard/0.5/shipyard/struct.World.html).

## Creation

You can use [`new`](https://docs.rs/shipyard/0.5/shipyard/struct.World.html#method.new) or [`default`](https://docs.rs/shipyard/0.5/shipyard/struct.World.html#method.default):

```rust, noplaypen
{{#include ../../../../tests/book/world.rs:world_new}}
```

There is no need to register components, storages are created on first access.

## Views

While some actions are available directly on [`World`](https://docs.rs/shipyard/0.5/shipyard/struct.World.html), you'll often interact with it through views. They allow access to one or multiple storage.  
Storage access follows the same rules as Rust's borrowing: You can have as many shared accesses to a storage as you like or a single exclusive access.

You can request a view using [`World::borrow`](https://docs.rs/shipyard/0.4.1/shipyard/struct.World.html#method.borrow), [`World::run`](https://docs.rs/shipyard/0.4.1/shipyard/struct.World.html#method.run) or in workloads (more on this in a later chapter).

For example if you want a shared access to the entities storage you can use [`borrow`](https://docs.rs/shipyard/0.4.1/shipyard/struct.World.html#method.borrow):

```rust, noplaypen
{{#include ../../../../tests/book/world.rs:view}}
```
