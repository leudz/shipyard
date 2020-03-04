# Borrow the World

`borrow` is one of three ways to modify components and entities.
It takes a single generic argument that lets us request access to storage(s):

```rust, noplaypen
let mut all_storages = world.borrow::<AllStorages>();
```

Storage access via borrowing follows the same rules as Rust's borrowing: you can have as many shared accesses to a storage as you like or a single unique access. `AllStorages` is special in that it is a unique access to all storages, so it prevents access to all other storages, even the entities.

When `borrow` executes, it will borrow at runtime (like a `RefCell`) all the requested storages plus a shared access to `AllStorages` and return what we call "views." This is why asking for `AllStorages` prevents unique access to anything else - otherwise you'd have shared and unique access to `AllStorages`. Importantly, this runtime borrow happens at each call. While borrowing is cheap, you should avoid calling `borrow` repeatedly in a hot loop.  If you're using a view multiple times, store it in a variable.

You can request multiple accesses using a tuple:
```rust, noplaypen
let (mut entities, mut empties) = world.borrow::<(EntitiesMut, &mut Empty)>();
```

We're using `EntitiesMut` and what looks like a reference to `Empty` coming from nowhere. Shipyard knows how to interpret this type parameter as a request for shared access to a storage.

Some of the possible values for these type parameters are (we'll see even more later):
- `AllStorages` - unique access to a storage that contains all other storages
- `Entities` - shared access to the entities storage
- `EntitiesMut` - same as above, but unique access
- `&T` - shared access to the storage holding `T`s
- `&mut T` - same as above, but unique access

We'll see what we can do with each of these in the following chapters. With this knowledge you'll be able to tell which one you need to access.

---

Thanks to `borrow` we can add entities and components to the `World`!
