# Iterators

Iteration is one of the most important features of an ECS.

In Shipyard this is achieved using the [`iter`](https://docs.rs/shipyard/latest/shipyard/trait.IntoIter.html#tymethod.iter) method on views.

### Single component type

You can use it on a single view to get one type of components:

```rust, noplaypen
world.run(|u32s: View<u32>| {
    for i in u32s.iter() {
        dbg!(i);
    }
});
```

This iterator is extremely fast, since all `u32` components are stored directly next to each other (which is something computers really like nowadays).

Unlike in the last chapter, there is no need to provide an [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html).
On the contrary, you can ask the iterator to tell you which entity each component is attached to using [`with_id`](https://docs.rs/shipyard/latest/shipyard/trait.Shiperator.html#method.with_id):

```rust, noplaypen
world.run(|u32s: View<u32>| {
    for (id, i) in u32s.iter().with_id() {
        println!("{} belongs to entity {:?}", i, id);
    }
});
```

### Multiple component types

While single views are useful, multiple views are where an ECS shines:

```rust, noplaypen
world.run(|u32s: View<u32>, usizes: View<usize>| {
    for (i, j) in (&u32s, &usizes).iter() {
        // -- snip --
    }
});
```

The iterator will only yield components from entities that have both `u32` and `usize` components, while ignoring the rest.

You can use views in any order.
However, using the same combination of views in different positions might yield components in a different order.
You shouldn't expect specific ordering from Shipyard's iterators, in general.

---

Iteration is only one of the main assets of an ECS.
In the next chapter we'll talk about perhaps the most important feature: Systems.
