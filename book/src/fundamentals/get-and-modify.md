# Get and Modify Components

To access or update component(s) of a single entity you can use [`get`](https://docs.rs/shipyard/latest/shipyard/trait.Get.html#tymethod.get). It'll work with both shared and unique views.

### Update a single component

```rust, noplaypen
world.run(|mut u32s: ViewMut<u32>| {
    *(&mut u32s).get(entity_id) = 1;
});
```

[`get`](https://docs.rs/shipyard/latest/shipyard/trait.Get.html#tymethod.get) will return an `&T` when used with a [`&View<T>`](https://docs.rs/shipyard/latest/shipyard/struct.View.html) and an `&mut T` with a [`&mut ViewMut<T>`](https://docs.rs/shipyard/latest/shipyard/struct.ViewMut.html). You can also get an `&T` from a [`ViewMut<T>`](https://docs.rs/shipyard/latest/shipyard/struct.ViewMut.html), which is why we have to explicitly mutably borrow `u32s`.

For single views if you're sure the entity has the component you want, you can index into it:

```rust, noplaypen
world.run(|mut u32s: ViewMut<u32>| {
    u32s[entity_id] = 1;
});
```

### Update multiple components

We can mix and match shared and unique component access with [`get`](https://docs.rs/shipyard/latest/shipyard/trait.Get.html#tymethod.get):

```rust, noplaypen
world.run(|mut u32s: ViewMut<u32>, usizes: View<usize>| {
        let (i, &j) = (&mut u32s, &usizes).get(entity_id);
        *i += j as u32;
        *i += j as u32;
    });
```
