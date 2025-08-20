# Get and Modify Components

To access or update components you can use [`Get::get`](https://docs.rs/shipyard/0.9/shipyard/trait.Get.html#tymethod.get). It'll work with both shared and exclusive views.

```rust, noplaypen
{{#include ../../../../tests/book/get.rs:get}}
```

When using a single view, if you are certain an entity has the desired component, you can access it via index.
