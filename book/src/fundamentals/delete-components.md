# Delete Components

### Delete a single component

```rust, noplaypen
world.run(|mut u32s: ViewMut<u32>| {
    u32s.delete(entity_id);
});
```

### Delete multiple components

```rust, noplaypen
world.run(|mut u32s: ViewMut<u32>, mut usizes: ViewMut<usize>| {
    Delete::<(u32, usize)>::delete((&mut u32s, &mut usizes), entity_id);
});
```

### Delete all components

Note that when you delete all components of an entity with [`strip`](https://docs.rs/shipyard/latest/shipyard/struct.AllStorages.html#method.strip), the entity itself won't be deleted. You can attach components to it again afterwards.

```rust, noplaypen
world.run(|mut all_storages: AllStoragesViewMut| {
    all_storages.strip(entity_id);
});
```
