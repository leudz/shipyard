# Other Components and Storages

So far we've only seen how to interact with `Send + Sync` components using the default storage, that's not always what we need.

### `!Send` and `!Sync` Components

`World` can store `!Send` and/or `!Sync` components once the corresponding feature is set but they come with limitations:

- `!Send` storages can only be added in `World`'s thread.
- `Send + !Sync` components can only be accessed from one thread at a time.
- `!Send + Sync` components can only be accessed immutably from other threads.
- `!Send + !Sync` components can only be accessed in the thread they were added in.

With these constrains you can run into issues like unrunnable systems or undeletable entities, be careful.
As a rule of thumb, try to call `World::run_default` and `run_workload` from `World`'s thread.

To help with `!Send` storages, all systems borrowing `AllStorages` will run in the thread `World::run_default` or `run_workload` is called in.

These storages are accessed with `NonSend`, `NonSync` and `NonSendSync`, for example:
```rust, noplaypen
#[system(Counted)]
fn run(rcs: NonSendSync<&Rc<usize>>) {}
```

### Unique Storages

When we known there'll only ever be exactly one instance of some component, it doesn't need to be attached to an entity. It also works well as global data while still being sound.

As opposed to other storages, unique storages have to be initialized with `add_unique`. This will both create the storage and initialize its only component. We can then access this component with `Unique`.  
Example:
```rust, noplaypen
let world = World::new();
world.add_unique(Camera::new());
let camera = world.borrow::<Unique<&Camera>>();
```

Note that `Unique` and `!Send`/`!Sync` components can be used together, in this case `Unique` will envelop `NonSend`/`NonSync` or `NonSendSync`.

### Tag Components

Components don't always need data, they're sometimes just there to flag entities. We can use empty structs to take care of this job.  
Example:
```rust, noplaypen
struct Dirty;
#[system(FlagDirty)]
fn run(dirties: &mut Dirty) { ... }
```
