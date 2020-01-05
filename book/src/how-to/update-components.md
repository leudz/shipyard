# Update Components 

To access or update the components of a single entity you can use `get`. It'll work with both shared and unique views.

### Update a single component

```rust, noplaypen
world.run::<&mut Position, _, _>(|mut positions| {
    *(&mut positions).get(entity_id).unwrap() = Position {
        x: 5.0,
        y: 6.0,
    };
});
```

`get` will return an `Option<&T>` when used with a `View<T>` and an `Option<&mut T>` with a `ViewMut<T>`. You can also get an `Option<&T>` from a `ViewMut<T>`, that's why we have to explicitly mutably borrow `positions`.

### Update a bunch of components

We can also mix and match shared and unique:

```rust, noplaypen
world.run::<(&mut Position, &Velocity), _, _>(|(mut positions, velocities)| {
    if let Some((pos, vel)) = (&mut positions, &velocities).get(entity_id) {
        *pos.x += *vel.x;
        *pos.y += *vel.y;
    }
});
```
