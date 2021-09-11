# Delete Components

Deleting a component will erase it from the storage but will not return it.

## World

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity((0u32, 1usize));

world.delete_component::<(u32,)>(id);
world.delete_component::<(u32, usize)>(id);
```

⚠️ We have to use a single element tuple `(T,)` to delete a single component entity.

#### All Components

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity((0u32, 1usize));

world.strip(id);
```

## View

We have to import the [`Delete`](https://docs.rs/shipyard/0.5.0/shipyard/trait.Delete.html) trait for multiple components.

```rust, noplaypen
let world = World::new();

let (mut entities, mut u32s, mut usizes) = world
    .borrow::<(EntitiesViewMut, ViewMut<u32>, ViewMut<usize>)>()
    .unwrap();

let id = entities.add_entity((&mut u32s, &mut usizes), (0, 1));

u32s.delete(id);
(&mut u32s, &mut usizes).delete(id);
```

#### All Components

```rust, noplaypen
let world = World::new();

let mut all_storages = world.borrow::<AllStoragesViewMut>().unwrap();

let id = all_storages.add_entity((0u32, 1usize));

all_storages.strip(id);
```
