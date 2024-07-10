# Remove Components

Removing a component will take it out of the storage and return it.

## World

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity((Pos::new(), Vel::new()));

world.remove::<Vel>(id);
world.remove::<(Pos, Vel)>(id);
```

## View

We have to import the [`Remove`](https://docs.rs/shipyard/latest/shipyard/trait.Remove.html) trait for multiple components.

```rust, noplaypen
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut vm_pos: ViewMut<Pos>, mut vm_vel: ViewMut<Vel>| {
        let id = entities.add_entity((&mut vm_pos, &mut vm_vel), (Pos::new(), Vel::new()));

        vm_pos.remove(id);
        (&mut vm_pos, &mut vm_vel).remove(id);
    },
);
```
