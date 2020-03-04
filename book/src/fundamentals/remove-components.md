# Remove Components

For components, "remove" and "delete" have different meanings. A remove returns the component(s) being removed.  A delete doesn't return anything.

### Remove a single component

```rust, noplaypen
let mut counts = world.borrow::<&mut Count>();

let count = counts.remove(entity_id);
```

There is no need for `Entities` here. You can call the `remove` method directly on your component storage view, passing it the id of the entity to remove it from.

### Remove a bunch of components

```rust, noplaypen
let (mut counts, mut empties) = world.borrow::<(&mut Count, &mut Empty)>();

let (_count, _empty) = Remove::<(Count, Empty)>::remove((&mut counts, &mut empties), entity_id);
```

We have to use the explicit syntax in this case because we could be trying to remove just `Count`. We'll see later why we'd want that.
