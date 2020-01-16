# Add Components 

An Entity can only contain one instance of a particular component type.

However there's no need to check for this - adding a component to an entity will simply replace the component if it already exists.

### Add a single component to an entity

```rust, noplaypen
let (entities, mut positions) = world.borrow::<(Entities, &mut Positions)>();

entities.add_component(
    &mut positions,
    Position { x: 0.0, y: 10.0 },
    entity_id,
);
```

You'll notice that we use `Entities` and not `EntitiesMut` it's because it's just here to make sure the id is alive and not deleted. We could use `EntitiesMut` of course but it's not needed.

Then it's exactly like `add_entity`, we pass the storage and the component. We also need an id this time since we're not creating it.

### Add a bunch of components to an entity

For multiple components we use a tuple just like `add_entity`.

```rust, noplaypen
let (entities, mut positions, mut fruits) = world.borrow::<(Entities, &mut Positions, &mut Fruit)>();

entities.add_component(
    (&mut positions, &mut fruits),
    (Position { x: 0.0, y: 10.0 }, Fruit::new_orange()),
    entity_id,
);
```
