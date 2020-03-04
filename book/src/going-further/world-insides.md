# What's inside a World?

In the last section we learned how to interact with `World` and access what's inside it without knowing what it is. Let's change that!

### Storage

Currently there exists only one type of storage in Shipyard: `SparseSet`. We're just going to focus on that implementation.

`SparseSet` is a data-structure made of 3 vectors:
- `sparse` contains indices (`usize`) to the `dense` vector
- `dense` contains indices (`EntityId`) to the `sparse` vector
- `data` contains the actual components

When `sparse` and `dense` point to each other, the entity owns the component present in the `data` vector at the same `dense` index.

`dense` is always the same length as `data`, the number of components present in the storage.

`sparse`, on the other hand, is more or less as big as the total number of entities created minus the number of entities deleted.

Let's look at an example:
```rust, noplaypen
let world = World::new();
let (mut entities, mut u32s, mut f32s) = world.borrow::<(EntitiesMut, &mut u32, &mut f32)>();
let entity0 = entities.add_entity(&mut u32s, 10);
let entity1 = entities.add_entity(&mut f32s, 20.0);
let entity2 = entities.add_entity(&mut u32s, 30);
```
When we create the `World` there is no component storage in it, `borrow` will create an empty storage for `u32` and `f32`.
We then create `entity0`, `u32`'s storage now looks like this:
```
sparse: [0]
dense: [0]
data: [10]
```
`sparse[0]` and `dense[0]` point to each other, the entity owns a component in this storage.
We add two more entities, now `u32` looks like this:
```
sparse: [0, 0, 1]
dense: [0, 2]
data: [10, 30]
```
We can see that `sparse[1]` got initialized with `0`, `entity1` doesn't have any `u32` component and `dense[0]` doesn't point back to it, all good.

### Entities

`Entities` is a simpler data structure, it's made of an `EntityId` vector and an optional tuple of two indices. This tuple points to the first and last deleted entity.

Each time an entity is added, the tuple is checked. If it is `None`, then the entity is allocated at the end of the vector.

If the tuple is `Some`, we'll use the oldest deleted index for the new entity and update the tuple.

If `EntityId` was interpreted as only an index, then two entities could have the same id. In just few operations, add - remove - add, we're back to the same index for a different entity, which could cause problems.

Which is why `EntityId` is not just an index, it is interpreted as two parts: a 48-bit index and a 16-bit version.

When we delete an entity its version gets incremented and its index becomes part of the optional tuple.

This tuple only contains two elements, however, so we use the indices of deleted `EntityID` entries to to form a linked list of all the deleted entries from most recently deleted to oldest.

Let's modify our previous example a little:
```rust, noplaypen
let world = World::new();
let entity0;
let entity1;
{
    let (mut entities, mut u32s, mut f32s) = world.borrow::<(EntitiesMut, &mut u32, &mut f32)>();
    entity0 = entities.add_entity(&mut u32s, 10);
    entity1 = entities.add_entity(&mut f32s, 20.0);
    let entity2 = entities.add_entity(&mut u32s, 30);
}
let mut all_storages = world.borrow::<AllStorages>();
all_storages.delete(entity0);
all_storages.delete(entity1);
```

Let's take a look at what happens to `Entities` as we run this code.  After adding all three entries, it looks something like this:
```
ids: [
    { index: 0, version: 0 },
    { index: 1, version: 0 },
    { index: 2, version: 0 }
]
deleted: None
```
Then we delete `entity0`. Since there is just a single deleted entity, it's both the most recent one and the oldest one in the tuple.

```
ids: [
    { index: 0, version: 1 },
    { index: 1, version: 0 },
    { index: 2, version: 0 }
]
deleted: Some((0, 0))
```
Finally, we delete `entity1`.  `ids[oldest].index` becomes `1` and we have a linked list where you start at the oldest index `0`.  `0` is not the newest index, so we know its value `1` is the index of the next deleted `EntryID`.  `1` is the newest index, so we know we have reached the end of the linked list of deleted entries.
```
ids: [
    { index: 1, version: 1 },
    { index: 1, version: 1 },
    { index: 2, version: 0 }
]
deleted: Some((1, 0))
```

### EntityId

While only 64 bits, `EntityId`s are very interesting. 48 bits are used for the index, and the remaining 16 for the version.

Almost all ECS have this kind of id, the difference being the length of the id and version.

32 bits felt too small to fit the wildest use cases. A generic approach was discarded due to: first - adding a generic everywhere, and second - making actions between worlds more difficult.

In some ECS implementations versions sometimes take more space, because who needs 48 bits for the index? But at the same time, who needs more than 16 bits for the version?

In the exceptional event that you add and remove entities to and from the same index enough times to reach the version limit, the `World` won't stop. This index will be considered dead (simply by not adding it the linked list) and you'll get an entity at another index on your next add.

Plus, we add and delete from opposite sides of the linked list making the version increase slower in general.

---

In the next chapter we'll look into parallelism.
