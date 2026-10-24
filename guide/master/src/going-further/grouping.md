# Grouping

Grouping is a way to order components across storages to improve iteration speed at the cost of insertion/removal.

## How it works

Shipyard uses `SparseSet` to store non unique components.
To iterate multiple storages, iterators usually pick one, go through `0..len` and for each component check in the other `SparseSet` if the entity has a component too.
For example, let's take `View<Sprite>` and `View<Transform>`. When we iterate them with `(&v_sprite, &v_transform).iter()`, one of the two `SparseSet` will be picked (the shortest).

```md
v_sprite:    [e0 e1 e2 e3]
v_transform: [e9 e3 e5 e7 e6]
```

Here we have `v_sprite` as the shortest so we'll use it.
Going in order, we check `e0`, `e1`, `e2`, `e3` and for each one, we check if the entity also has a component in `v_transform`.
Turns out, only `e3` has both components but we had to iterate all 4 entities.

Grouping declares sets of storages so entities matching the set have their components aligned.
This way, we avoid many entities and visit components linearly in memory.

If we go back to our example.
Let's say we declare a `Sprite+Transform` group. Then we insert the `Sprite` and `Transform` components as above.

```txt
v_sprite:    [e0 e1 e2 e3]
v_transform: [e9 e3 e5 e7 e6]
```

Nothing has changed yet. Now we can call `World::regroup`.

```txt
v_sprite:    [e3 e1 e2 e0]
v_transform: [e3 e9 e5 e7 e6]
```

After a `regroup`, components that are part of a group get aligned.
We also have metadata that tells us the `Sprite+Transform` length is 1.
When we iterate again, we don't need to check anything. We can retrieve `e3` and stop.

## Declaration

Groups must be declared when the `World` is created.
This restriction allows workloads to automatically insert `regroup_set` calls.

```rs
{{#include ../../../../tests/book/grouping.rs:declaration}}
```

A single component can be grouped multiple times.
In that case, union groups will appear when entities belong to multiple sets.
The more overlapping groups, the more fragmented iteration becomes.

Here, if an entity has all `Sprite+Transform+SpriteAnimation`, a new union will be created.

## Usage

Grouping doesn't require any special `iter` call.

```rs
{{#include ../../../../tests/book/grouping.rs:iter}}
```

Iteration is always correct but adding/removing components without `regroup` will reduce iteration speed.
Workloads automatically handle `regroup` calls.

Outside workloads, make sure to `regroup` or `regroup_set` somewhere between adding/removing components and iterating grouped storages.