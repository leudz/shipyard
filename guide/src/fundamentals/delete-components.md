# Delete Components

Deleting a component will erase it from the storage but will not return it.

## World

### Single Component

```rust, noplaypen
{{#include ../../../tests/book/delete_components.rs:world_one}}
```

⚠️ We have to use a single element tuple.

### Multiple Components

```rust, noplaypen
{{#include ../../../tests/book/delete_components.rs:world_multiple}}
```

### All Components

```rust, noplaypen
{{#include ../../../tests/book/delete_components.rs:world_all}}
```

## View

### Single Component

```rust, noplaypen
{{#include ../../../tests/book/delete_components.rs:view_one}}
```

### Multiple Components

We have to import the [`Delete`](https://docs.rs/shipyard/latest/shipyard/trait.Delete.html) trait for this one.

```rust, noplaypen
{{#include ../../../tests/book/delete_components.rs:view_multiple}}
```

### All Components

```rust, noplaypen
{{#include ../../../tests/book/delete_components.rs:view_all}}
```
