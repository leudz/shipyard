# Add Components 

An entity can only have a single instance of a particular component type.  
Adding a second component of the same type to an entity will simply replace the existing component.

### Add a single component

```rust, noplaypen
world.run(|entities: EntitiesView, mut u32s: ViewMut<u32>| {
    entities.add_component(entity_id, &mut u32s, 0);
});
```

You'll notice that we use [`EntitiesView`](https://docs.rs/shipyard/latest/shipyard/struct.EntitiesView.html) and not [`EntitiesViewMut`](https://docs.rs/shipyard/latest/shipyard/struct.EntitiesViewMut.html).
The entities storage is only used to check if the [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html) is alive.
We could of course use [`EntitiesViewMut`](https://docs.rs/shipyard/latest/shipyard/struct.EntitiesViewMut.html), but exclusive access is not necessary.

Just like [`add_entity`](https://docs.rs/shipyard/latest/shipyard/struct.Entities.html#method.add_entity), we pass the storage and the component value.

### Add multiple components

We use tuples for multiple components just as with [`add_entity`](https://docs.rs/shipyard/latest/shipyard/struct.Entities.html#method.add_entity).

```rust, noplaypen
world.run(
    |entities: EntitiesView, mut u32s: ViewMut<u32>, mut usizes: ViewMut<usize>| {
        entities.add_component(entity_id, (&mut u32s, &mut usizes), (0, 10));
    },
);
```
