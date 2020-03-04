# Add Entities

To add entities we'll use the `EntitiesViewMut` view for unique access to entities storage.

### Add an entity with a single component

```rust, noplaypen
let (mut entities, mut empties) = world.borrow::<(EntitiesMut, &mut Empty)>();

let entity = entities.add_entity(&mut empties, Empty);
```

`add_entity` creates a new entity and adds it to the storage.  In order to add components to the entity at creation time, `add_entity` takes two arguments: a unique reference to a mutable view of the component storage, and the actual component value.

This will return an `EntityId`, the id of the newly created entity.

### Add an entity with multiple components

We can make an entity with multiple components, of course! For that we'll just have to use tuples for both arguments:

```rust, noplaypen
let (mut entities, mut empties, mut counts) = world.borrow::<(EntitiesMut, &mut Empty, &mut Count)>();

let entity = entities.add_entity((&mut empties, &mut counts), (Empty, Count(0)));
```

### Add an entity with no components

We can use `()` for both the view and the value to create an empty entity and add components to it later.

```rust, noplaypen
let entity = world.borrow::<EntitiesMut>().add_entity((), ());
```
