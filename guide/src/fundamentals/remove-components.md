# Remove Components

Removing a component will take it out of the storage and return it.

## World

### Single Component

```rust, noplaypen
{{#include ../../../tests/book/remove_components.rs:world_one}}
```

⚠️ We have to use a single element tuple.

### Multiple Components

```rust, noplaypen
{{#include ../../../tests/book/remove_components.rs:world_multiple}}
```

## View

### Single Component

```rust, noplaypen
{{#include ../../../tests/book/remove_components.rs:view_one}}
```

### Multiple Components

We have to import the [`Remove`](https://docs.rs/shipyard/latest/shipyard/trait.Remove.html) trait for this one.

```rust, noplaypen
{{#include ../../../tests/book/remove_components.rs:view_multiple}}
```
