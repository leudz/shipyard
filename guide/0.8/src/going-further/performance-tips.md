# Performance Tips

List of small information to get the most out of Shipyard.

### `for_each`

`for ... in` desugars to calling [`next`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#tymethod.next) repeatedly, the compiler can sometimes optimize it very well.  
If you don't want to take any chance prefer calling [`for_each`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.for_each) instead.

### `borrow` / `run` in a loop

While borrowing storages is quite cheap, doing so in a loop is generally a bad idea.  
Prefer moving the loop inside [`run`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html#method.run) and move [`borrow`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html#method.borrow)'s call outside the loop.

### `bulk_add_entity`

When creating many entities at the same time remember to call [`bulk_add_entity`](https://docs.rs/shipyard/0.8/shipyard/struct.World.html#method.bulk_add_entity) if possible.

### Deleting entities

This is a niche optimization but the methods presented in the [Delete Components](../fundamentals/delete-components.md) chapter are not always the fastest way to delete an entity.

When an entity is deleted, all storages have to be checked to delete the components of that entity.
But if you know which components this entity might have, you can focus the search on those and ignore the other storages.

Instead of calling [World::delete_entity](https://docs.rs/shipyard/0.8/shipyard/struct.World.html#method.delete_entity) or [AllStorages::delete_entity](https://docs.rs/shipyard/0.8/shipyard/struct.AllStorages.html#method.delete_entity) you can call `delete` on all potential storages using the [Delete](https://docs.rs/shipyard/0.8/shipyard/trait.Delete.html) trait and [Entities::delete_unchecked](https://docs.rs/shipyard/0.8/shipyard/struct.Entities.html#method.delete_unchecked).
