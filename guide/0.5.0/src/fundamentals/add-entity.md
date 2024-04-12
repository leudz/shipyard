# Add Entity

When an entity is created you will receive a unique handle to it: an [`EntityId`](https://docs.rs/shipyard/0.5.0/shipyard/struct.EntityId.html).

## World

```rust, noplaypen
let mut world = World::new();

let empty_entity = world.add_entity(());
let single_component = world.add_entity((0u64,));
let multiple_components = world.add_entity((0u64, 1usize));
```

⚠️ We have to use a single element tuple `(T,)` to add a single component entity.

## Views

```rust, noplaypen
let world = World::new();

let (mut entities, mut u64s, mut usizes) = world
    .borrow::<(EntitiesViewMut, ViewMut<u64>, ViewMut<usize>)>()
    .unwrap();

let empty_entity = entities.add_entity((), ());
let single_component = entities.add_entity(&mut u64s, 0);
let multiple_components = entities.add_entity((&mut u64s, &mut usizes), (0, 1));
```
