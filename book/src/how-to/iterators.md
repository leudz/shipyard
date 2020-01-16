# Iterators

Fast iteration is one of the most important features of an ECS.

In Shipyard this is achieved using the `iter` method on view(s).

### Single component

You can use it on a single view:

```rust, noplaypen
let positions = world.borrow::<&Position>();

(&positions).iter().for_each(|pos| {
    dbg!(pos);
});
```

This iterator will go through all `Position` components, very fast since they're all stored one next to the other, something computer really like nowadays.

Unlike last chapter, no need to provide any `EntityId`. It's even the opposite, you can ask the iterator to tell you which entity each component is attached to using `with_id`:

```rust, noplaypen
let positions = world.borrow::<&Position>();

(&positions).iter().with_id().for_each(|(id, pos)| {
    println!("Entity {:?} is at {:?}", id, pos);
});
```

### Multiple components

While single views are useful, multiple views is where ECS shines:

```rust, noplaypen
let (positions, fruits) = world.borrow::<(&Position, &Fruit)>();

(&positions, &fruits).iter().for_each(|(pos, fruit)| {
    println!("There is a {:?} at {:?}", pos, fruit);
});
```

The iterator will only yield components from entities that have both `Position` and `Fruit` while ignoring the other.

You can use views in any order but the same combination with the views in different positions might not yield components in the same order. In general you shouldn't expect any order from iterators.

---

Iteration is only one of the main assets of an ECS, in the next chapter we'll take about another one, probably the most important one.
