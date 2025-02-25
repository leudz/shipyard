# Delete Entity

Deleting an entity deletes it from the entities storage, while also deleting all its components.

## World

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity(Pos::new());

world.delete_entity(id);
```

## View

```rust, noplaypen
let world = World::new();

world.run(|mut all_storages: AllStoragesViewMut| {
    let id = all_storages.add_entity(Pos::new());

    all_storages.delete_entity(id);
});
```
