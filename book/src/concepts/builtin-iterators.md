# Builtin Iterators

Let's say we have a dozen or so components that we need to iterate over, but in separate groupings.

For example, let's say we want to do the following steps:

1. iterate over all the Positions and Velocities. Given the current Position and Velocity, update Position.
2. iterate over all the Positions and Meshes in order to render them.

Note that in the first step, there is no need for Mesh and in the second step there is no need for Velocity. Yet Position is needed in both.

Shipyard uses Rust's trait system to create Iterators for any components that are registered in the World.

Once a system is registered and the storages are passed in (see [Storages, Views, and Entities](./storages-views-and-entities.md)), it's as simple as:

```rust,noplaypen
(&mut positions, &velocities).iter().for_each(|(position, velocity)| {
    //...
});
(&positions, &velocities).iter().for_each(|(position, velocity)| {
    //...
});
```