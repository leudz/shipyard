# Add Entities

To add entities we'll use the view to the entities' storage: `EntitiesViewMut`.

### Add an entity with a single component

```rust, noplaypen
let (mut entities, mut empties) = world.borrow::<(EntitiesMut, &mut Empty)>();

let entity = entities.add_entity(&mut empties, Empty);
```

`add_entity` takes two arguments, a unique reference to the view you want to add a component to and the component.

It'll return an `EntityId`, a handle to the newly created entity.

### Add an entity with multiple components

We can make an entity with multiple components of course, for that we'll just have to use tuples for both arguments:

```rust, noplaypen
let (mut entities, mut empties, mut counts) = world.borrow::<(EntitiesMut, &mut Empty, &mut Count)>();

let entity = entities.add_entity((&mut empties, &mut counts), (Empty, Count(0)));
```

### Add an entity with no component

We can use `()` to create an empty entity and add components to it later.

```rust, noplaypen
let entity = world.borrow::<EntitiesMut>().add_entity((), ());
```
