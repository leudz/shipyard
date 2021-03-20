# Add Entity

When an entity is created you will receive a unique handle to it: an [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html).

## World

```rust, noplaypen
{{#include ../../../../tests/book/add_entity.rs:world}}
```

⚠️ We have to use a single element tuple `(T,)` to add a single component entity.

## Views

```rust, noplaypen
{{#include ../../../../tests/book/add_entity.rs:view}}
```
