# Add Entities

To add entities we'll use [`EntitiesViewMut`](https://docs.rs/shipyard/latest/shipyard/struct.EntitiesViewMut.html), the exclusive view over the entities storage, and [`ViewMut`](https://docs.rs/shipyard/latest/shipyard/struct.ViewMut.html), an exclusive view over a component storage.

### Add an entity with a single component

```rust, noplaypen
world.run(|mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>| {
    let _entity = entities.add_entity(&mut u32s, 0);
});
```

[`add_entity`](https://docs.rs/shipyard/latest/shipyard/struct.Entities.html#method.add_entity) creates a new entity with the given component and it'll return an [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html), the id of the newly created entity.

### Add an entity with multiple components

Of course, we can also make an entity with multiple components. For that we'll just have to use tuples for both arguments:

```rust, noplaypen
world.run(
    |mut entities: EntitiesViewMut, mut u32s: ViewMut<u32>, mut usize: ViewMut<usize>| {
        let _entity = entities.add_entity((&mut u32s, &mut usize), (0, 10));
    },
);
```

### Add an entity with no components

We can use `()` for both argument to create an empty entity and add components later.

```rust, noplaypen
world.run(|mut entities: EntitiesViewMut| {
    let _entity = entities.add_entity((), ());
});
```
