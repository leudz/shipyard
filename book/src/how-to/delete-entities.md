# Delete Entities

Deleting an entity means take it away from the `Entities` storage as well as deleting all its components.

```rust, noplaypen
world.run::<AllStorages, _, _>(|mut all_storages| {
    all_storages.delete(entity_id);
});
```

`delete` takes a single `EntityId` of the entity you want to delete. `delete` returns a `bool`, `true` if the entity was present in the `Entities` storage before calling the method.
