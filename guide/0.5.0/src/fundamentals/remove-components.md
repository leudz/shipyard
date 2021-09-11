# Remove Components

Removing a component will take it out of the storage and return it.

## World

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity((0u32, 1usize));

world.remove::<(u32,)>(id);
world.remove::<(u32, usize)>(id);
```

⚠️ We have to use a single element tuple `(T,)` to remove a single component entity.

## View

We have to import the [`Remove`](https://docs.rs/shipyard/0.5.0/shipyard/trait.Remove.html) trait for multiple components.

```rust, noplaypen
let world = World::new();

let (mut entities, mut u32s, mut usizes) = world
    .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<usize>)>()
    .unwrap();

let id = entities.add_entity((&mut u32s, &mut usizes), (0, 1));

u32s.remove(id);
(&mut u32s, &mut usizes).remove(id);
```
