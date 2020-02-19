# Packs

`SparseSet`s are very flexible, so much so that we can use their internal layout to encode additional information.

### Tight

Let's start with the one offering the most performance, so good your CPU will stop taking so many vacations.  
Tight pack moves components of an entity that contains all components in the pack to the front of the `data` vector. The nice thing is: a single storage contains all components of this type so it's always tightly packed.  
To tightly pack multiple storages we use the `tight_pack` method on multiple views.  
Let's go through an example:
```rust, noplaypen
let world = World::new();

let (mut entities, mut usizes, mut u32s) = world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();
(&mut usizes, &mut u32s).tight_pack();

let entity0 = entities.add_entity(&mut usizes, 0);
let entity1 = entities.add_entity((&mut usizes, &mut u32s), (1, 11));
```
After adding `entity0`, `usize`'s storage looks like this (version omitted):
```
sparse: [0]
dense: [0]
data: [0]
```
Let's add `entity1`:
```
sparse: [1, 0]
dense: [1, 0]
data: [1, 0]
```
We still have `sparse` and `dense` looking at each other and `entity1` has its component at index `0` since it contains both `usize` and `u32` as opposed to `entity0`.

This is very powerful because when we iterate `usize` and `u32` we don't have to check anything. We know all entities matching these components are in the range `0..=last_packed`.

There's also one more advantage, we can iterate over chunks of these components with `into_chunk` and `into_chunk_exact`.

The big downside is we have to access and pass all packed storages when doing any action on the storages that might affect the order. Adding/removing or deleting components for example. This is also the reason `Remove` and `Delete` look so bad.

### Loose

Packs comes with one more limitation: we can't pack multiple times the same storage ([for now](https://github.com/leudz/shipyard/issues/47)).  
Loose pack let us pack again a tightly packed storage or just pack two storages without shuffling one of them.  
This time we'll need the storages we want to pack, the other storages and call `loose_pack`.  
Example:
```rust, noplaypen
let world = World::new();

let (mut entities, mut usizes, mut u32s) = world.borrow::<(EntitiesMut, &mut usize, &mut u32)>();
LoosePack::<(usize,)>::loose_pack((&mut usizes, &mut u32s));

let entity0 = entities.add_entity(&mut usizes, 0);
let entity1 = entities.add_entity((&mut usizes, &mut u32s), (1, 11));
```
Here we packed `usize`'s storage while `u32` isn't affected.  
After both entities have been added this is what `usize`'s storage look like:
```
sparse: [1, 0]
dense: [1, 0]
data: [1, 0]
```

While the iteration speed isn't as good as tight pack, it's still better than no pack at all.  

### Update

This pack doesn't increase iteration speed but rather slows it down. That's not its purpose however, it records which components have been *inserted*, *modified* and *deleted*.  
To get these functionalities, use `update_pack` on a single storage.  
The storage will start recording *inserted* components, we can then access them with `inserted` and `inserted_mut`. Note that mutating a component in the *inserted* section won't put it in the *modified* section. To move the *inserted* components we have to call `clear_inserted`, this will move them in the *neutral* section.  
*modified* components can be accessed with `modified`/`modified_mut` and cleared with `clear_modified`. *modified* is a bit misleading, components are flagged as soon as they're mutably accessed.  
Be careful, removed components won't be recorded, only *deleted* ones. They can be accessed with `deleted`, it contains both the components and the id of the entity they belonged to.  
We also can take ownership of these deleted components with `take_deleted`.

### Free

Free pack isn't implemented yet ([#56](https://github.com/leudz/shipyard/issues/56)), it'll use an external vector to keep track of which entities have the required storages, speeding up iteration while not reordering any storage.

---

In the next chapter we'll look into `!Send`/`!Sync` components and unique storage.
