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

let (count, empty) = (&mut counts, &mut empties).remove(entity_id);
```

### Delete all components

We're deleting again and not removing, note that the entity won't be deleted, you'll be able to attach components to it again.

```rust, noplaypen
let all_storages = world.borrow::<AllStorages>();

all_storages.strip(entity_id);
```
