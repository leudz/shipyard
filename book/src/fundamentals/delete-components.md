# Delete Components

### Delete a single component

```rust, noplaypen
let mut counts = world.borrow::<&mut Count>();

counts.delete(entity_id);
```

### Delete a bunch of components

```rust, noplaypen
let (mut counts, mut empties) = world.borrow::<(&mut Count, &mut Empty)>();

Delete::<(Count, Empty)>::delete((&mut counts, &mut empties), entity_id);
```

### Delete all components

Note that when you delete all an entity's components with `strip`, the entity itself won't be deleted. You can attach components to it again afterwards.

```rust, noplaypen
let mut all_storages = world.borrow::<AllStorages>();

all_storages.strip(entity_id);
```
