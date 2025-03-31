# Delete Components

Deleting a component will erase it from the storage but will not return it.

## World

```rust, noplaypen
{{#include ../../../../tests/book/delete_components.rs:world}}
```

#### All Components

```rust, noplaypen
{{#include ../../../../tests/book/delete_components.rs:world_all}}
```

## View

We have to import the [`Delete`](https://docs.rs/shipyard/0.8/shipyard/trait.Delete.html) trait for multiple components.

```rust, noplaypen
{{#include ../../../../tests/book/delete_components.rs:view}}
```

#### All Components

```rust, noplaypen
{{#include ../../../../tests/book/delete_components.rs:view_all}}
```
