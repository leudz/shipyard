# Remove Components

We use the word "remove" and not "delete", not because it would be repetitive but because these two operations have different meaning. A delete won't return anything while a remove will return the component(s).

### Remove a single component

```rust, noplaypen
world.run::<&mut Count, _, _>(|mut counts| {
    let (count,) = (&mut counts,).remove(entity_id);
});
```

No need for `Entities` here, you can call the method directly on the view and give the id of the entity. But we always have to use a tuple, see the [syntactic peculiarities chapter](../concepts/syntactic-peculiarities.md) if you want to know why.

### Remove a bunch of components

```rust, noplaypen
world.run::<(&mut Count, &mut Empties), _, _>(|(mut counts, mut empties)| {
    let (count, empty) = (&mut counts, &mut empties).remove(entity_id);
});
```

### Delete all components

We're deleting again and not removing, note that the entity won't be deleted, you'll be able to attach components to it again.

```rust, noplaypen
world.run::<AllStorages, _, _>(|mut all_storages| {
    all_storages.strip(entity_id);
});
```
