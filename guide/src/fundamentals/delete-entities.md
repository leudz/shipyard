# Delete Entities

Deleting an entity deletes it from the entities storage, while also deleting all its components.

```rust, noplaypen
world.run(|mut all_storages: AllStoragesViewMut| {
    all_storages.delete_entity(entity_id);
}).unwrap();
```

[`delete`](https://docs.rs/shipyard/latest/shipyard/struct.AllStorages.html#method.delete_entity) takes a single [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html) of the entity you want to delete.
The return value is a `bool` that indicates whether the entity existed in the entities storage.
