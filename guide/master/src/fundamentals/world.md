# World

[`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) is Shipyard's core data structure: It holds all data and knows how to process systems. All operations originate from one (or more) [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html).

## Creation

You can use [`new`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.new) or [`default`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.default):

```rust, noplaypen
{{#include ../../../../tests/book/world.rs:world_new}}
```

There is no need to register components, storages are created on first access.

## Views

While some actions are available directly on [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html), you'll often interact with it through views. They allow access to one or multiple storage.  
Storage access follows the same rules as Rust's borrowing: as many shared accesses to a storage as you like or a single exclusive access.

You can request a view using [`World::run`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run), [`World::borrow`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.borrow) or with workloads (more on this in a later chapter).\
These three methods have the exact same storage access abilities.\
[`borrow`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.borrow) has the extra ability to allow fallible storage access while workloads are about system composition.\
Most examples in this guide require neither so we'll use almost exclusively [`run`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run).

For example if you want a shared access to the entities storage:

```rust, noplaypen
{{#include ../../../../tests/book/world.rs:view}}
```
