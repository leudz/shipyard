# Iterators

Iteration is one of the most important features of an ECS.

## World

```rust, noplaypen
let world = World::new();

for (i, j) in &mut world.iter::<(&mut Pos, &Vel)>() {
    i.0 += j.0;
}
```

THe "extra" `&mut` is unfortunate but necessary.

## Views

Iteration on views is achieved using [`IntoIter::iter`](https://docs.rs/shipyard/latest/shipyard/trait.IntoIter.html#tymethod.iter).

```rust, noplaypen
let world = World::new();

world.run(|mut vm_pos: ViewMut<Pos>, v_vel: View<Vel>| {
    for i in vm_pos.iter() {
        dbg!(i);
    }
    
    for (i, j) in (&mut vm_pos, &v_vel).iter() {
        i.0 += j.0;
    }
});
```

You can use views in any order. However, using the same combination of views in different positions may yield components in a different order.  
You shouldn't expect specific ordering from Shipyard's iterators in general.

#### With Id

You can ask an iterator to tell you which entity owns each component by using [`WithId::with_id`](https://docs.rs/shipyard/latest/shipyard/trait.IntoWithId.html#method.with_id):

```rust, noplaypen
let world = World::new();

world.run(|v_pos: View<Pos>| {
    for (id, i) in v_pos.iter().with_id() {
        println!("{:?} belongs to entity {:?}", i, id);
    }
});
```

#### Not

It's possible to filter entities that don't have a certain component using [`Not`](https://docs.rs/shipyard/latest/shipyard/struct.Not.html) by adding `!` in front of the view reference.

```rust, noplaypen
let world = World::new();

world.run(|v_pos: View<Pos>, v_vel: View<Vel>| {
    for (i, _) in (&v_pos, !&v_vel).iter() {
        dbg!(i);
    }
});
```
