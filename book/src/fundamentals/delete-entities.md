# Delete Entities

Deleting an entity deletes it from the entities storage as well as deleting all its components.

```rust, noplaypen
world.run(|mut all_storages: AllStoragesViewMut| {
    all_storages.delete(entity_id);
});
```

[`delete`](https://docs.rs/shipyard/latest/shipyard/struct.AllStorages.html#method.delete) takes a single [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html) of the entity you want to delete. It returns a `bool` indicating if the entity was present in the entities storage.
