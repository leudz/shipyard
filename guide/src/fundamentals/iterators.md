# Iterators

Iteration is one of the most important features of an ECS.

In Shipyard this is achieved using [`IntoIter::iter`](https://docs.rs/shipyard/latest/shipyard/trait.IntoIter.html#tymethod.iter) on views.

```rust, noplaypen
{{#include ../../../tests/book/iterators.rs:iter}}
```

You can use views in any order. However, using the same combination of views in different positions might yield components in a different order.  
You shouldn't expect specific ordering from Shipyard's iterators in general.

#### With Id

You can ask an iterator to tell you which entity owns each component by using [`WithId::with_id`](https://docs.rs/shipyard/latest/shipyard/trait.IntoWithId.html#method.with_id):

```rust, noplaypen
{{#include ../../../tests/book/iterators.rs:with_id}}
```

#### Not

It's possible to filter entities that don't have a certain component using [`Not`](https://docs.rs/shipyard/latest/shipyard/struct.Not.html) by adding `!` in front of the view reference.

```rust, noplaypen
{{#include ../../../tests/book/iterators.rs:not}}
```
