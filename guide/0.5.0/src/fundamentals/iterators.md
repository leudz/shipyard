# Iterators

Iteration is one of the most important features of an ECS.

In Shipyard this is achieved using [`IntoIter::iter`](https://docs.rs/shipyard/0.5.0/shipyard/trait.IntoIter.html#tymethod.iter) on views.

```rust, noplaypen
let world = World::new();

let (mut u64s, usizes) = world.borrow::<(ViewMut<u64>, View<usize>)>().unwrap();

for i in u64s.iter() {
    dbg!(i);
}

for (mut i, j) in (&mut u64s, &usizes).iter() {
    *i += *j as u64;
}
```

You can use views in any order. However, using the same combination of views in different positions might yield components in a different order.  
You shouldn't expect specific ordering from Shipyard's iterators in general.

#### With Id

You can ask an iterator to tell you which entity owns each component by using [`WithId::with_id`](https://docs.rs/shipyard/0.5.0/shipyard/trait.IntoWithId.html#method.with_id):

```rust, noplaypen
let world = World::new();

let u64s = world.borrow::<View<u64>>().unwrap();

for (id, i) in u64s.iter().with_id() {
    println!("{} belongs to entity {:?}", i, id);
}
```

#### Not

It's possible to filter entities that don't have a certain component using [`Not`](https://docs.rs/shipyard/0.5.0/shipyard/struct.Not.html) by adding `!` in front of the view reference.

```rust, noplaypen
let world = World::new();

let (u64s, usizes) = world.borrow::<(View<u64>, View<usize>)>().unwrap();

for (i, _) in (&u64s, !&usizes).iter() {
    dbg!(i);
}
```
