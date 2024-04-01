# `!Send` and `!Sync` Components

[`World`](https://docs.rs/shipyard/0.5.0/shipyard/struct.World.html) can store `!Send` and/or `!Sync` components once the `thread_local` feature is set but they come with limitations:

- `!Send` storages can only be added in [`World`](https://docs.rs/shipyard/0.5.0/shipyard/struct.World.html)'s thread.
- `Send + !Sync` components can only be accessed from one thread at a time.
- `!Send + Sync` components can only be accessed immutably from other threads.
- `!Send + !Sync` components can only be accessed in the thread they were added in.

These storages are accessed with [`NonSend`](https://docs.rs/shipyard/0.5.0/shipyard/struct.NonSend.html), [`NonSync`](https://docs.rs/shipyard/0.5.0/shipyard/struct.NonSync.html) and [`NonSendSync`](https://docs.rs/shipyard/0.5.0/shipyard/struct.NonSendSync.html), for example:

```rust, noplaypen
fn run(rcs_usize: NonSendSync<View<Rc<usize>>>, rc_u64: NonSendSync<UniqueView<Rc<u64>>>) {}
```
