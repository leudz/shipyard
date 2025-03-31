# Iterators

Iteration is one of the most important features of an ECS.

## World

```rust, noplaypen
{{#include ../../../../tests/book/iterators.rs:world}}
```

The "extra" `&mut` is unfortunate but necessary.

## Views

Iteration on views is achieved using [`IntoIter::iter`](https://docs.rs/shipyard/0.8/shipyard/trait.IntoIter.html#tymethod.iter).

```rust, noplaypen
{{#include ../../../../tests/book/iterators.rs:iter}}
```

You can use views in any order. However, using the same combination of views in different positions may yield components in a different order.  
You shouldn't expect specific ordering from Shipyard's iterators in general.

#### With Id

You can ask an iterator to tell you which entity owns each component by using [`WithId::with_id`](https://docs.rs/shipyard/0.8/shipyard/trait.IntoWithId.html#method.with_id):

```rust, noplaypen
{{#include ../../../../tests/book/iterators.rs:with_id}}
```

#### Not

It's possible to filter entities that don't have a certain component using [`Not`](https://docs.rs/shipyard/0.8/shipyard/struct.Not.html) by adding `!` in front of the view reference.

```rust, noplaypen
{{#include ../../../../tests/book/iterators.rs:not}}
```
