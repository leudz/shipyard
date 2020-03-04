# Get and Modify Components

To access or update component(s) of a single entity you can use `get`. It'll work with both shared and unique views.

### Update a component of a single entity

```rust, noplaypen
let mut positions = world.borrow::<&mut Position>();

*(&mut positions).get(entity_id).unwrap() = Position {
    x: 5.0,
    y: 6.0,
};
```

`get` will return an `Option<&T>` when used with a `View<T>` and an `Option<&mut T>` with a `ViewMut<T>`. You can also get an `Option<&T>` from a `ViewMut<T>`, which is why we have to explicitly mutably borrow `positions`.

For single views if you're sure the entity has the component you want, you can index into it:

```rust, noplaypen
let mut positions = world.borrow::<&mut Position>();

positions[entity_id] = Position {
    x: 5.0,
    y: 6.0,
};
```

### Update multiple components of a single entity

We can also mix and match shared and unique component access with `get`:

```rust, noplaypen
let (mut positions, velocities) = world.borrow::<(&mut Position, &Velocity)>();

if let Ok((pos, vel)) = (&mut positions, &velocities).get(entity_id) {
    pos.x += vel.x;
    pos.y += vel.y;
}
```
