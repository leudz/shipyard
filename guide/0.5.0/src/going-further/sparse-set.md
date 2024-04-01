# Sparse Set

[`SparseSet`](https://docs.rs/shipyard/0.5.0/shipyard/struct.SparseSet.html) is Shipyard's default storage. This chapter explains the basics of how it works, the actual implementation is more optimized both in term of speed and memory.

### Overview

[`SparseSet`](https://docs.rs/shipyard/0.5.0/shipyard/struct.SparseSet.html) is made of three arrays:

- `sparse` contains indices to the `dense` and `data` arrays
- `dense` contains [`EntityId`](https://docs.rs/shipyard/0.5.0/shipyard/struct.EntityId.html)
- `data` contains the actual components

`dense` and `data` always have the same length, the number of components present in the storage.  
`sparse` on the other hand can be as big as the total number of entities created.

Let's look at an example:

```rust, noplaypen
let mut world = World::new();

let entity0 = world.add_entity((0u64,));
let entity1 = world.add_entity((10.0f32,));
let entity2 = world.add_entity((20u64,));
```

The [`World`](https://docs.rs/shipyard/0.5.0/shipyard/struct.World.html) starts out empty, when we add `0u64` a [`SparseSet<u64>`](https://docs.rs/shipyard/0.5.0/shipyard/struct.SparseSet.html) will be generated.

At the end of the example we have:

```txt
SparseSet<u64>:
    sparse: [0, dead, 1]
    dense:  [0, 2]
    data:   [0, 20]

SparseSet<f32>:
    sparse: [dead, 0]
    dense:  [1]
    data:   [10.0]
```

You can see that [`SparseSet<u64>`](https://docs.rs/shipyard/0.5.0/shipyard/struct.SparseSet.html)'s `sparse` contains three elements but `dense` does not.  
Note also that both `sparse` don't contains the same number of elements. As far as [`SparseSet<f32>`](https://docs.rs/shipyard/0.5.0/shipyard/struct.SparseSet.html) knowns `entity2` might not exist.

### Removal

Removing is done by swap removing from both `dense` and `data` and updating `sparse` in consequence.

Continuing the previous example if we call:

```rust, noplaypen
world.remove::<(u64,)>(entity0);
```

The internal representation now looks like this:

```txt
sparse: [dead, dead, 0]
dense: [2]
data: [20]
```

`dense` and `data` shifted to the left, `sparse`'s first element is now dead and the third element is now `0` to follow `dense`'s shift.

### Iteration

Iterating one or several [`SparseSet`](https://docs.rs/shipyard/0.5.0/shipyard/struct.SparseSet.html) is different. With a single [`SparseSet`](https://docs.rs/shipyard/0.5.0/shipyard/struct.SparseSet.html) it's as simple as iterating `data`.  
To iterate multiple [`SparseSet`](https://docs.rs/shipyard/0.5.0/shipyard/struct.SparseSet.html)s the smallest will be chosen as "lead". We then iterate its `dense` array and for each entity we check all the other [`SparseSet`](https://docs.rs/shipyard/0.5.0/shipyard/struct.SparseSet.html)s to see if they also contain a component for this entity.
