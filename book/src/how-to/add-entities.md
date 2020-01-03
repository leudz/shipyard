# Add Entities

### Add an entity with no components

```rust
world.run::<EntitiesMut, _, _>(|mut entities| {
    let entity_id = entities.add_entity((), ());
});
```

We are borrowing the entity storage mutably, then we can call `add_entity` it and it'll return an `EntityId` that we can use to refer to this entity.


### Add a single entity

```code examples and explanation here```
