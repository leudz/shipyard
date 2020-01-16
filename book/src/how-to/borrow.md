# Borrow the World

`borrow` is one of three ways to modify components and entities.

We already saw it in last chapter but here's another invocation:

```rust, noplaypen
let (mut entities, mut empties) = world.borrow::<(EntitiesMut, &mut Empty)>();
```

This time no `AllStorages` but a [tuple](../concepts/syntactic-peculiarities.md) of `EntitiesMut` and what looks like a reference to `Empty` coming from nowhere. So let's take this generic apart!

The generic argument will let you request storages and return their views. Here's an exhaustive list of all types you can use:
- `AllStorages` -> `AllStoragesViewMut` - unique access to a storage that contains the other storages
- `Entities` -> `EntitiesView` - shared access of the storage holding the entities
- `EntitiesMut` -> `EntitiesViewMut` - same as above but unique access
- `&T` -> `View<T>` - shared access to the storage holding `T`s
- `&mut T` -> `ViewMut<T>` - same as above but unique access
- `Not<&T>` -> `Not<View<T>>` - same as `&T` but will skip entities that have this component when used in an iterator
- `Not<&mut T>` -> `Not<ViewMut<T>>` - same as above but unique access
- `Unique<&T>` -> `UniqueView<T>` - shared access to a `T` unique storage
- `Unique<&mut T>` -> `UniqueViewMut<T>` - same as above but unique access
- `ThreadPool` - shared access to the thread pool used by systems

You'll learn more about what each of them is capable of in the following chapters. With this knowledge you'll be able to tell which one you need to access.

These accesses follow the same rules as Rust's borrowing, you can have as many shared accesses to a storage as you like or a single unique access. `AllStorages` being the exception, it prevents access to all other storages, even `Entities` and `EntitiesMut`.

When `borrow` executes it will borrow at runtime (like a `RefCell`) all the requested storages plus a shared access to `AllStorages`. This is why asking for `AllStorages` means you can't ask for anything else - otherwise you'd have a shared and a unique access to `AllStorages`. Importantly, this runtime borrow happens at each call, while it's cheap if you're using a view multiple times, better store it in a variable than calling `borrow` repeatedly. Example, instead of doing:

```rust, noplaypen
world.borrow::<AllStorages>().register::<usize>();
world.borrow::<AllStorages>().register::<u32>();
```
better do:
```rust, noplaypen
let mut all_storages = world.borrow::<AllStorages>();
all_storages.register::<usize>();
all_storages.register::<u32>();
```

---

In the next chapter we'll start to do something with views!
