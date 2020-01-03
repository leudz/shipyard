# Create a World and Storages

`World` is Shipyard's core data-structure, it holds all data, knows how to process systems and all operations originate from one (or more).

You can create one by using `default`:

```rust, noplaypen
let world = World::default();
```

or `new` if you want it to come with pre-registered components:

```rust, noplaypen
let world = World::new::<(usize, i32)>();
```

Registering a component is important, it'll create a storage for it, which in turn will allow you to perform all actions you'd expect from an ECS.

Note that we didn't make `world` mutable, it's because all `World`'s methods take a shared reference. This makes `World` easier to use across threads.

So far so good but sometimes you'll want to register even more storages, there's two way to do it:

### Register with `World`

```rust, noplaypen
let world = World::default();

world.register::<(usize,)>();
```

When registering with `World` you always have to give a tuple, even for a single type like in this case. This may feel a bit weird and your instincts would be right, there's a paragraph explaining why it has to be this way in [this chapter](../concepts/syntactic-weirdness.md).

### Register with `AllStorages`

`AllStorages` is what holds all components inside `World`. You can access it with a system or `run` like in the following example:

```rust, noplaypen
let world = World::default();

world.run::<AllStorages, _, _>(|mut all_storages| {
    all_storages.register::<usize>();
});
```

This time no tuple, `register` when used with `AllStorages` takes only a single type.

The `run` method is intimidating but don't worry at the end of next chapter you'll know everything about it!
