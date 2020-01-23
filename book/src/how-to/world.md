# Create a World and Storages

`World` is Shipyard's core data-structure, it holds all data, knows how to process systems and all operations originate from one (or more).

You can create one by using `default` or `new`:

```rust, noplaypen
let world = World::default();
let world = World::new();
```

No need to register components their storage will be created when we access them. 

Note that we didn't make `world` mutable, it's because all `World`'s methods take a shared reference. This makes `World` easier to use across threads.

---

Now that we have a `World` that would be nice to be able to do something with it, it's what we'll see in the next chapter!
