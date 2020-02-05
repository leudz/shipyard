# Remove Components

We use the word "remove" and not "delete", not because it would be repetitive but because these two operations have different meaning. A delete won't return anything while a remove will return the component(s).

### Remove a single component

```rust, noplaypen
let mut counts = world.borrow::<&mut Count>();

let count = counts.remove(entity_id);
```

No need for `Entities` here, you can call the method directly on the view and give the id of the entity.

### Remove a bunch of components

```rust, noplaypen
let (mut counts, mut empties) = world.borrow::<(&mut Count, &mut Empty)>();

let (_count, _empty) = Remove::<(Count, Empty)>::remove((&mut counts, &mut empties), entity_id);
```

We have to use the explicit syntax in this case because we could be trying to remove just `Count`. We'll see later why we'd want that.
