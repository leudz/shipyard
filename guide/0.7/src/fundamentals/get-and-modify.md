# Get and Modify Components

To access or update components you can use [`Get::get`](https://docs.rs/shipyard/latest/shipyard/trait.Get.html#tymethod.get). It'll work with both shared and exclusive views.

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity((Pos::new(), Vel::new()));

world.run(|mut vm_pos: ViewMut<Pos>, mut vm_vel: ViewMut<Vel>| {
    (&mut vm_vel).get(id).unwrap().0 += 1.0;

    let (mut i, j) = (&mut vm_pos, &vm_vel).get(id).unwrap();
    i.0 += j.0;

    vm_pos[id].0 += 1.0;
});
```

When using a single view, if you are certain an entity has the desired component, you can access it via index.
