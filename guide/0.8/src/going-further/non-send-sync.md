# `!Send` and `!Sync` Components

[`World`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html) can store `!Send` and/or `!Sync` components once the `thread_local` feature is set but they come with limitations:

- `!Send` storages can only be added in [`World`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html)'s thread.
- `Send + !Sync` components can only be accessed from one thread at a time.
- `!Send + Sync` components can only be accessed immutably from other threads.
- `!Send + !Sync` components can only be accessed in the thread they were added in.

These storages are accessed with [`NonSend`](https://docs.rs/shipyard/0.8/shipyard/struct.NonSend.html), [`NonSync`](https://docs.rs/shipyard/0.8/shipyard/struct.NonSync.html) and [`NonSendSync`](https://docs.rs/shipyard/0.8/shipyard/struct.NonSendSync.html), for example:

```rust, noplaypen
{{#include ../../../../tests/book/non_send_sync.rs:non_send_sync}}
```
