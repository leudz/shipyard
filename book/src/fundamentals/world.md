# Create a World

`World` is Shipyard's core data structure: it holds all data and knows how to process systems. All operations originate from one (or more) `World`s.

You can create a `World` by using `default` or `new`:

```rust, noplaypen
let world = World::default();
let world = World::new();
```

There is no need to register components. A component's storage will be created when we access it. 

Note that we didn't make `world` mutable. That is because all `World`'s methods take a shared reference. This makes `World` easier to use across threads.

---

Now that we have a `World`, it would be nice to be able to do something with it. That's what we'll see in the next chapter!
