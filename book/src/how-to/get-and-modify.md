# Get and Modify Components 

To access or update the components of a single entity you can use `get`. It'll work with both shared and unique views.

### Update a single component

```rust, noplaypen
let mut position = world.borrow::<&mut Position>();

*(&mut positions).get(entity_id).unwrap() = Position {
    x: 5.0,
    y: 6.0,
};
```

`get` will return an `Option<&T>` when used with a `View<T>` and an `Option<&mut T>` with a `ViewMut<T>`. You can also get an `Option<&T>` from a `ViewMut<T>`, that's why we have to explicitly mutably borrow `positions`.

For single views and if you're sure the entity has the component you want, you can index into it:

```rust, noplaypen
let mut position = world.borrow::<&mut Position>();

positions[entity_id] = Position {
    x: 5.0,
    y: 6.0,
};
```

### Update a bunch of components

We can also mix and match shared and unique:

```rust, noplaypen
let (positions, velocities) = world.borrow::<(&mut Position, &mut Velocity)>();

if let Some((pos, vel)) = (&mut positions, &velocities).get(entity_id) {
    *pos.x += *vel.x;
    *pos.y += *vel.y;
}
```
