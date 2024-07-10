# Add Components

An entity can have any number of components but only one in each storage.  
Adding another component of the same type will replace the existing one.

## World

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity(());

world.add_component(id, Vel::new());
world.add_component(id, (Pos::new(), Vel::new()));
```

## View

When adding components, the entities storage is only used to check if the [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html) is alive.  
We don't need exclusive access to the entities storage.

If you don't need to check if the entity is alive, you can use the [`AddComponent`](https://docs.rs/shipyard/latest/shipyard/trait.AddComponent.html) trait and do without the entities storage entirely.

```rust, noplaypen
let world = World::new();

world.run(
    |mut entities: EntitiesViewMut, mut vm_pos: ViewMut<Pos>, mut vm_vel: ViewMut<Vel>| {
        let id = entities.add_entity((), ());

        entities.add_component(id, &mut vm_pos, Pos::new());
        entities.add_component(id, (&mut vm_pos, &mut vm_vel), (Pos::new(), Vel::new()));
        vm_vel.add_component_unchecked(id, Vel::new());
    },
);
```
