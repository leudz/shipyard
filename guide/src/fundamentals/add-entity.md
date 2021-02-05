# Add Entity

When an entity is created you will receive a unique handle to it: an [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html).

## World

### Empty Entity

```rust, noplaypen
{{#include ../../../tests/book/add_entity.rs:world_empty}}
```

### Single Component Entity

```rust, noplaypen
{{#include ../../../tests/book/add_entity.rs:world_one}}
```

⚠️ We have to use a single element tuple.

### Multiple Components Entity

```rust, noplaypen
{{#include ../../../tests/book/add_entity.rs:world_multiple}}
```

## Views

### Empty Entity

```rust, noplaypen
{{#include ../../../tests/book/add_entity.rs:view_empty}}
```

### Single Component Entity

```rust, noplaypen
{{#include ../../../tests/book/add_entity.rs:view_one}}
```

### Multiple Components Entity

```rust, noplaypen
{{#include ../../../tests/book/add_entity.rs:view_multiple}}
```
