# Add Components

An entity can have any number of components but only one in each storage.  
Adding another component of the same type will replace the existing one.

## World

### Single Component

```rust, noplaypen
{{#include ../../../tests/book/add_components.rs:world_one}}
```

⚠️ We have to use a single element tuple.

### Multiple Components

```rust, noplaypen
{{#include ../../../tests/book/add_components.rs:world_multiple}}
```

## View

You'll notice that we use [`EntitiesView`](https://docs.rs/shipyard/latest/shipyard/struct.EntitiesView.html) and not [`EntitiesViewMut`](https://docs.rs/shipyard/latest/shipyard/struct.EntitiesViewMut.html) to add components.  
The entities storage is only used to check if the [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html) is alive.  
We could of course use [`EntitiesViewMut`](https://docs.rs/shipyard/latest/shipyard/struct.EntitiesViewMut.html), but exclusive access is not necessary.

### Single Component

```rust, noplaypen
{{#include ../../../tests/book/add_components.rs:view_one}}
```

### Multiple Components

```rust, noplaypen
{{#include ../../../tests/book/add_components.rs:view_multiple}}
```
