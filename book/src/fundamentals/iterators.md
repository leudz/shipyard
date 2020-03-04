# Iterators

Fast iteration is one of the most important features of an ECS.

In Shipyard this is achieved using the `iter` method on view(s).

### Single component type

You can use it on a single view to get one type of components:

```rust, noplaypen
let positions = world.borrow::<&Position>();

(&positions).iter().for_each(|pos| {
    dbg!(pos);
});
```

This iterator will go through all `Position` components very fast since they're all stored one next to each other, which is something computers really like nowadays.

Unlike in the last chapter, there is no need to provide an `EntityId`. On the contrary, you can ask the iterator to tell you which entity each component is attached to using `with_id`:

```rust, noplaypen
let positions = world.borrow::<&Position>();

(&positions).iter().with_id().for_each(|(id, pos)| {
    println!("Entity {:?} is at {:?}", id, pos);
});
```

### Multiple component types

While single views are useful, multiple views are where an ECS shines:

```rust, noplaypen
let (positions, fruits) = world.borrow::<(&Position, &Fruit)>();

(&positions, &fruits).iter().for_each(|(pos, fruit)| {
    println!("There is a {:?} at {:?}", fruit, pos);
});
```

The iterator will only yield components from entities that have both `Position` and `Fruit` components, while ignoring the rest.

You can use views in any order, but the same combination with the views in different positions might yield components in a different order. In general you shouldn't expect specific ordering from iterators.

---

Iteration is only one of the main assets of an ECS, in the next chapter we'll talk about perhaps the most important one: Systems.
