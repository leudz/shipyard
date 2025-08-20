# Add Components

An entity can have any number of components but only one in each storage.  
Adding another component of the same type will replace the existing one.

## World

```rust, noplaypen
{{#include ../../../../tests/book/add_components.rs:world}}
```

## View

When adding components, the entities storage is only used to check if the [`EntityId`](https://docs.rs/shipyard/0.9/shipyard/struct.EntityId.html) is alive.  
We don't need exclusive access to the entities storage.

If you don't need to check if the entity is alive, you can use the [`AddComponent`](https://docs.rs/shipyard/0.9/shipyard/trait.AddComponent.html) trait and do without the entities storage entirely.

```rust, noplaypen
{{#include ../../../../tests/book/add_components.rs:view}}
```
