# Remove Components

For components, "remove" and "delete" have different meanings. A remove returns the component(s) being removed.  A delete doesn't return anything.

### Remove a single component

```rust, noplaypen
world.run(|mut u32s: ViewMut<u32>| {
    let i = u32s.remove(entity_id);
});
```

There is no need for `Entities` here. You can call the `remove` method directly on the component storage view.

### Remove multiple components

```rust, noplaypen
world.run(|mut u32s: ViewMut<u32>, mut usizes: ViewMut<usize>| {
    let (i, j) = Remove::<(u32, usize)>::remove((&mut u32s, &mut usizes), entity_id);
});
```

We have to use the explicit syntax in this case because we could be trying to remove just `u32`. We'll see later why we'd want that.
