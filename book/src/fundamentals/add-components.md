# Add Components 

An entity can only have a single instance of a particular component type.

Adding a second component of the same type to an entity will simply replace the existing component.

### Add a single component to an entity

```rust, noplaypen
let (entities, mut positions) = world.borrow::<(Entities, &mut Position)>();

entities.add_component(
    &mut positions,
    Position { x: 0.0, y: 10.0 },
    entity_id,
);
```

You'll notice that we use `Entities` and not `EntitiesMut`, because the entities storage is only used to look and see if the entity id is alive (not deleted). We could use `EntitiesMut` of course, but unique access is not necessary.

Just like with `add_entity`, we pass the storage and the component value. We also need an entity id this time to specify an existing entity.

### Add multiple components to an entity

For multiple components we use tuples just like we did with `add_entity`.

```rust, noplaypen
let (entities, mut positions, mut fruits) = world.borrow::<(Entities, &mut Position, &mut Fruit)>();

entities.add_component(
    (&mut positions, &mut fruits),
    (Position { x: 0.0, y: 10.0 }, Fruit::new_orange()),
    entity_id,
);
```
