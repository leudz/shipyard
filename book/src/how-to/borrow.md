# Borrow the World

`borrow` is one of three ways to modify components and entities.\
It has a single generic argument that let us request access to storage(s):

```rust, noplaypen
let mut all_storages = world.borrow::<AllStorages>();
```

These accesses follow the same rules as Rust's borrowing, you can have as many shared accesses to a storage as you like or a single unique access. `AllStorages` being the exception, it's a unique access and prevents access to all other storage, even to the entities.

When `borrow` executes it will borrow at runtime (like a `RefCell`) all the requested storages plus a shared access to `AllStorages` and return what we call views. This is why asking for `AllStorages` means you can't ask for anything else - otherwise you'd have a shared and a unique access to `AllStorages`. Importantly, this runtime borrow happens at each call, while it's cheap if you're using a view multiple times, you should store it in a variable rather than calling `borrow` repeatedly.

You can request multiple accesses using a tuple:
```rust, noplaypen
let (mut entities, mut empties) = world.borrow::<(EntitiesMut, &mut Empty)>();
```

We're using `EntitiesMut` and what looks like a reference to `Empty` coming from nowhere. This reference is just a code that Shipyard knows to interpret.

This code works with all these types (plus a few more we'll see later):
- `AllStorages` - unique access to a storage that contains the other storages
- `Entities` - shared access of the storage holding the entities
- `EntitiesMut` - same as above but unique access
- `&T` - shared access to the storage holding `T`s
- `&mut T` - same as above but unique access

We'll see what we can do with each of them in the following chapters. With this knowledge you'll be able to tell which one you need to access.

---

thanks to `borrow` we can add entities and components to the `World`!
