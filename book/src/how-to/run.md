# Run the World

`run` is one of two ways to modify components and entities.

We already saw it in last chapter but here's another invocation:

```rust, noplaypen
world.run::<(EntitiesMut, &mut Empty), _, _>(|(entities, empties)| { ... });
```

This time no `AllStorages` but a [tuple](../concepts/syntactic-peculiarities.md) of `EntitiesMut` and what looks like a reference to `Empty` coming from nowhere. So let's take this method call apart!

As you can see it's a method on `World` and it takes 3 generic parameters but the compiler can almost always guess the last two.

The first generic will let you request the storage(s) you want to access. Here's an exhaustive list of all types you can use:
- `AllStorages` - unique access to a storage that contains the other storages
- `Entities` - shared access of the storage holding the entities
- `EntitiesMut` - same as above but unique access
- `&T` - shared access to the storage holding `T`s
- `&mut T` - same as above but unique access
- `Not<&T>` - same as `&T` but will skip entities that have this component when used in an iterator
- `Not<&mut T>` - same as above but unique access
- `Unique<&T>` - shared access to a `T` unique storage
- `Unique<&mut T>` - same as above but unique access
- `ThreadPool` - shared access to the thread pool used by systems

You'll learn more about what each of them is capable of in the following chapters. With this knowledge you'll be able to tell which one you need to access.

These accesses follow the same rules as Rust's borrowing, you can have as many shared accesses to a storage as you like or a single unique access. `AllStorages` being the exception, it prevents access to any other storage, even `Entities` and `EntitiesMut`.

We then have our first underscore, it's the type returned by `run`, for example:

```rust, noplaypen
let entity = world.run::<(EntitiesMut, &mut Empty), _, _>(|(entities, empties)| { ... });
```

The last underscore is the complete type of the closure, `run`'s single parameter. This closure also takes a single parameter based on `run`'s first generic.

When `run` executes it will borrow at runtime (like a `RefCell`) all the requested storages plus a shared access to `AllStorages`. This is why asking for `AllStorages` means you can't ask for anything else - otherwise you'd have a shared and a unique access to `AllStorages` at the same time. Importantly, this runtime borrow is cheap and will only happen once per call so only merge `run` calls if it makes sense not for gaining performance.

What we get in the closure in called a view, it's most of the time the whole storage but can be just part of it in some situations.

In the next chapter we'll find out what this mysterious `...` really is!
