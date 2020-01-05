# Add Entities

Now that we know everything about `run`, it's time to use it!

### Add an entity with a single component

```rust, noplaypen
world.run::<(Entities, &mut Empties), _, _>(|(mut entities, mut empties)| {
    let entity_id = entities.add_entity(&mut empties, Empty);
});
```

There's a lot of `mut` in this `run` call, if you find it a bit weird there's a good explanation for it in [this chapter](../concepts/syntactic-weirdness.md).

`add_entity` takes two arguments, a unique reference to the storage you want to add a component to and the component.

It'll return an `EntityId`, a handle to the newly created entity.

### Add an entity with multiple components

You can make an entity with multiple components of course, for that you'll just have to put all arguments in tuples:

```rust, noplaypen
let entity = world.run::<(Entities, &mut Empties, &mut Count), _, _>(|(mut entities, mut empties, mut count)| {
    entities.add_entity((&mut empties, &mut count), (Empty, Count(0)))
});
```

We removed the `;` in the closure so we can return the id.

### Add an entity with no component

We can use `()` to create an empty entity and add components to it later.

```rust, noplaypen
world.run::<EntitiesMut, _, _>(|mut entities| {
    entities.add_entity((), ());
});
```
