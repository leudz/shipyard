# Remove Components

Removing a component will take it out of the storage and return it.

## World

```rust, noplaypen
{{#include ../../../../tests/book/remove_components.rs:world}}
```

⚠️ We have to use a single element tuple `(T,)` to remove a single component entity.

## View

We have to import the [`Remove`](https://docs.rs/shipyard/latest/shipyard/trait.Remove.html) trait for multiple components.

```rust, noplaypen
{{#include ../../../../tests/book/remove_components.rs:view}}
```
