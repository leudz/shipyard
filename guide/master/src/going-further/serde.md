# Serde

Serializing an ECS can be very easy or difficult, it all depends on what you're trying to achieve.\
For example, let's say we only want to serialize two components: `Player` and `Position`.\
We have many format options:

1. An array of `(EntityId, Player)` then another array of `(EntityId, Position)`.\
This could include entities that have either component or only the ones that have both.
2. An array of `(EntityId, (Player, Position))`.
3. An array of `EntityId` then an array of `(Player, Position)`.
4. An array of `EntityId` then another of `Player` then yet another of `Position`.
5. The list goes on.

So which option is the best? It depends on the use case.\
Option 1, for example, is one of the worst at memory but the best at retrieving random components.

There are as many options possible when deserializing: should components be overwritten? should `EntityId`s match? ...

## EntityId

Serializing [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html) is technically all that is needed to serialize all entities and components in a [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html).\
It will require lots of work on the user's side but is the most flexible.\
This will let us pick the best format for our use case.

[`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html) is very easy to (de)serialize.
Example:

```rs
{{#include ../../../../tests/book/serde.rs:entity_id}}
```

A `Vec<EntityId>` would be just as simple.

## Views

This is where our options become limited.\
If `shipyard` does the entire view(s) serialization, it has to pick a format.

The current implementation leaves the door open for future user customization.\
For now, only Option 1 is implemented. Each component will create an array of `(EntityId, Component)`.\
When deserializing, components will be attributed to the same [`EntityId`](https://docs.rs/shipyard/latest/shipyard/struct.EntityId.html) they were serialized with. They will override any existing component.

```rs
{{#include ../../../../tests/book/serde.rs:single_view}}
```

Serializing multiple components is not that much more work.

```rs
{{#include ../../../../tests/book/serde.rs:multiple_views}}
```