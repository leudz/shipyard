# Delete Components

Deleting a component will erase it from the storage but will not return it.

## World

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity((Pos::new(), Vel::new()));

world.delete_component::<Vel>(id);
world.delete_component::<(Pos, Vel)>(id);
```

#### All Components

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity((Pos::new(), Vel::new()));

world.strip(id);
```

## View

We have to import the [`Delete`](https://docs.rs/shipyard/latest/shipyard/trait.Delete.html) trait for multiple components.

```rust, noplaypen
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut vm_pos: ViewMut<Pos>, mut vm_vel: ViewMut<Vel>| {
        let id = entities.add_entity((&mut vm_pos, &mut vm_vel), (Pos::new(), Vel::new()));

        vm_pos.delete(id);
        (&mut vm_pos, &mut vm_vel).delete(id);
    },
);
```

#### All Components

```rust, noplaypen
let world = World::new();

world.run(|mut all_storages: AllStoragesViewMut| {
    let id = all_storages.add_entity((Pos::new(), Vel::new()));

    all_storages.strip(id);
});
```
