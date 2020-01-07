# Welcome to Shipyard!

[Shipyard](https://github.com/leudz/shipyard) is an Entity Component System focused on usability and speed. An ECS is a great way to organize logic and data.

There are two main benefits to using an ECS:

1. Elegant approach for humans
    - Composition over inheritance
    - Separation of concerns
    - Less burdened by lifetimes
2. Optimal design for computers
    - Spatial locality
    - Less pointer chasing

However, programming with an ECS requires thinking about data and logic in a different way than you might be used to. Also, the optimization techniques need explicit choice and tuning to get the greatest benefit (see [Packs](./concepts/packs.md))

# How does it work?

`EntityId` is just a newtype wrapping `u64`, it's just an index.

Components hold data. Only data. No logic. They _can_ of course contain methods but the ECS only sees the hard data.

Systems do the heavy lifting: updating components, running side-effects, and integrating with other parts of the code.

To know Shipyard differs from other ECS implementations, see [Different Types of ECS](./pilgrimage/different-types-of-ecs.md).
