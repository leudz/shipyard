# Get and Modify Components

To access or update components you can use [`Get::get`](https://docs.rs/shipyard/latest/shipyard/trait.Get.html#tymethod.get). It'll work with both shared and exclusive views.

```rust, noplaypen
{{#include ../../../tests/book/get.rs:get}}
```

When using a single view, if you're certain an entity has the desired component, you can access it via index.

### Fast Get

Using [`get`](https://docs.rs/shipyard/latest/shipyard/trait.Get.html#tymethod.get) with [`&mut ViewMut<T>`](https://docs.rs/shipyard/latest/shipyard/struct.ViewMut.html) will return a [`Mut<T>`](https://docs.rs/shipyard/latest/shipyard/struct.Mut.html). This struct help fine track component modification.  
[`FastGet::fast_get`](https://docs.rs/shipyard/latest/shipyard/trait.FastGet.html#tymethod.fast_get) can be used to opt out of this fine tracking and get back `&mut T`.