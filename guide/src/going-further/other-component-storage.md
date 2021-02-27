# Other Components and Storages

So far we've only seen how to interact with `Send + Sync` components using the default storage, that's not always what we need.

### `!Send` and `!Sync` Components

[`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) can store `!Send` and/or `!Sync` components once the corresponding feature is set but they come with limitations:

- `!Send` storages can only be added in [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html)'s thread.
- `Send + !Sync` components can only be accessed from one thread at a time.
- `!Send + Sync` components can only be accessed immutably from other threads.
- `!Send + !Sync` components can only be accessed in the thread they were added in.

These storages are accessed with [`NonSend`](https://docs.rs/shipyard/latest/shipyard/struct.NonSend.html), [`NonSync`](https://docs.rs/shipyard/latest/shipyard/struct.NonSync.html) and [`NonSendSync`](https://docs.rs/shipyard/latest/shipyard/struct.NonSendSync.html), for example:
```rust, noplaypen
fn run(rcs: NonSendSync<View<Rc<usize>>>) {}
```

### Unique Storages

When we know there'll only ever be exactly one instance of some component, it doesn't need to be attached to an entity. It also works well as global data while still being safe.

As opposed to other storages, unique storages have to be initialized with `add_unique`. This will both create the storage and initialize its only component. We can then access this component with [`UniqueView`](https://docs.rs/shipyard/latest/shipyard/struct.UniqueView.html) and [`UniqueViewMut`](https://docs.rs/shipyard/latest/shipyard/struct.UniqueViewMut.html).

```rust, noplaypen
let world = World::new();

world.add_unique(Camera::new());

world.run(|camera: UniqueView<Camera>| {
    // -- snip --
})
```

Note that `!Send`/`!Sync` components can be stored in unique storages.

### Tag Components

Components don't always need data, they're sometimes just there to flag entities. We can use empty structs to take care of this job.

Example:
```rust, noplaypen
struct Dirty;

fn flag_dirty(mut dirties: ViewMut<Dirty>) {
    // -- snip --
}
```
