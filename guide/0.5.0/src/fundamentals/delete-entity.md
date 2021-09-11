# Delete Entity

Deleting an entity deletes it from the entities storage, while also deleting all its components.

## World

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity((0u32,));

world.delete_entity(id);
```

## View

```rust, noplaypen
let world = World::new();

let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

let id = all_storages.add_entity((0u32,));

all_storages.delete_entity(id);
```
