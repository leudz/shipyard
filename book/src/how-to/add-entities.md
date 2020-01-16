# Add Entities

Now that we know everything about `borrow`, it's time to use it!

### Add an entity with a single component

```rust, noplaypen
let (mut entities, mut empties) = world.borrow::<(EntitiesMut, &mut Empties)>();

let entity = entities.add_entity(&mut empties, Empty);
```

`add_entity` takes two arguments, a unique reference to the storage you want to add a component to and the component.

It'll return an `EntityId`, a handle to the newly created entity.

### Add an entity with multiple components

You can make an entity with multiple components of course, for that you'll just have to put all arguments in tuples:

```rust, noplaypen
let (mut entities, mut empties, mut counts) = world.borrow::<(EntitiesMut, &mut Empties, &mut Count)>();

let entity = entities.add_entity((&mut empties, &mut counts), (Empty, Count(0)));
```

### Add an entity with no component

We can use `()` to create an empty entity and add components to it later.

```rust, noplaypen
let entity = world.borrow::<EntitiesMut>().add_entity((), ());
```
