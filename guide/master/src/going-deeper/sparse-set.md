# Sparse Set

[`SparseSet`][sparseset docs] is Shipyard's default storage. This chapter explains the basics
of how it works, the actual implementation is more optimized both in term of speed and memory.

### Overview

To understand how Shipyard uses sparse sets, we must first understand how sparse sets work.
A basic sparse set is a data structure for storing integers. It is comprised of two
arrays: `sparse` and `dense`.

To insert an integer `i`, we first set the next available slot in the `dense` array to `i`,
and then set `sparse[i]` to the position of `i` in the dense array. Let's walk through
an example.

We start off with an empty sparse set:

- Sparse Array: `[]`
- Dense Array: `[]`

To add `3` to our sparse set, we first append it to `dense` and then set `sparse[3]` to `0`
(the position of `3` in `dense`):

- Sparse Array: `[U, U, U, 0]`
- Dense Array: `[3]`
  `U` is short for uninitialised.

If we then add `0`, the sparse set will look like so:

- Sparse Array: `[1, U, U, 0]`
- Dense Array: `[3, 0]`

Searching a sparse set is `O(1)`. To check if the integer `i` exists we check whether
`dense[sparse[i]] == i`. For example, to look up `3` in our example sparse set, we should
first check `sparse[check]`. `sparse[check]` is equal to `0` and so next we check
`dense[0]`. Since `dense[0] == 3` we can say that `3` is in our example sparse set.

### Shipyard

So far, we've only seen how sparse sets can store integers. However, Shipyard has to store both
entity IDs (basically just integers) and components, requiring us to use a slightly more
complicated data structure. Shipyard makes two major changes to the traditional sparse set
described above.

Firstly, Shipyard sparse sets are actually composed of three arrays: `sparse`, `dense`, and
`data`. `dense` stores the entity IDs, whereas `data` contains the actual components of the
entities. `dense` and `data` are linked: their lengths are always the same. `data[i]` is
the component for the entity with the ID located at `dense[i]`. Whenever `dense` changes,
so does `data`.

Secondly, Shipyard uses multiple sparse sets, one for each type of component. The `dense` array
in each sparse set contains the [`EntityIds`][entityid docs] of the entities that have that
component.

Let's walk through an example:

```rust,noplaypen
{{#include ../../../../tests/book/sparse_set.rs:insertion}}
```

For this example we will assume that the entity IDs are in order i.e. `entity_id_0 == 0`, `entity_id_1 == 1`, etc.

The world data will now be stored in two sparse sets, one for each component:

```txt
SparseSet<FirstComponent>:
    sparse: [0, U, 1, 2]
    dense:  [0, 2, 3]
    data:   [FirstComponent(322), FirstComponent(5050), FirstComponent(958)]

SparseSet<SecondComponent>:
    sparse: [U, 0, 1]
    dense:  [1, 2]
    data:   [SecondComponent(17), SecondComponent(3154)]
```

`U` is short for uninitialised.

### Iteration

To iterate over a single sparse set, we can simply iterate over the `data` array.
However, Shipyard also lets us iterate over multiple sparse sets.

To iterate over multiple sparse sets, we first pick the shortest set (comparing the lengths
of the `dense` arrays) and then iterate over the `dense` array of the shortest set. For each
entity ID, we check whether all the other sparse sets contain it, and if they do, we yield
the entity ID in the iterator.

Let's walk through an example with the sparse set we defined above:

```rust,noplaypen
{{#include ../../../../tests/book/sparse_set.rs:iteration}}
```

We first check which has the shortest dense set. The `SecondComponent` sparse set does, so
we begin iterating over its `dense` array.

The first entity ID is `1`. Since we are iterating over `SecondComponent`, we already know
that entity `1` has a `SecondComponent`; we just need to check if the entity has a
`FirstComponent`. As described above, to check whether an entity has a component, we have
to check if `dense[sparse[id]] == id` in the sparse set of the component. `sparse[1]` in
`SparseSet<FirstComponent>` is uninitialised and so we know that entity `1` does not have
a `FirstComponent`.

The next entity that contains a `SecondComponent` is `2`. However, this time, `sparse[2]`
in `SparseSet<FirstComponent>` is equal to `1` and `dense[1]` is equal to `2`, which means
that entity `2` has a `FirstComponent` meaning we can yield it in the iterator.

After iterating over all the items in the `SecondComponent` sparse set, we are done.

### Removal

Removing is done by swap removing from both `dense` and `data` and updating `sparse` in
consequence.

Continuing the previous example if we call:

```rust,noplaypen
{{#include ../../../../tests/book/sparse_set.rs:removal}}
```

The internal representation now looks like this:

```txt
sparse: [U, U, 0, 1]
dense: [2, 3]
data: [FirstComponent(5050), FirstComponent(958)]
```

`dense` and `data` shifted to the left, the first element in sparse is now uninitialised,
and the indexes at `sparse[2]` and `sparse[3]` were updated.

### Additional Resources

[This blog post][skypjack blog post] goes into more detail on sparse sets and compares them
with archetypes, another common way of representing data in ECS libraries. The blog post is
part of a larger series about the design and internals of ECS systems.

[entityid docs]: https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html
[sparseset docs]: https://docs.rs/shipyard/latest/shipyard/struct.SparseSet.html
[skypjack blog post]: https://skypjack.github.io/2019-03-07-ecs-baf-part-2/
