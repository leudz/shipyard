# Welcome to Shipyard!

[Shipyard](https://github.com/leudz/shipyard) is an Entity Component System focused on usability and speed. ECS is a great way to organize logic and data.

There are two main benefits to using an ECS:

1. Elegant approach for humans
    - Composition over inheritance
    - Separation of concerns
    - Less burdened by lifetimes
2. Optimal design for computers
    - Spatial locality
    - Less pointer chasing

However, programming with an ECS requires thinking about data and logic in a different way than you might be used to.

# How does it work?

Components hold data. Entities are simple ids used to refer to a group of components.

Systems do the heavy lifting: updating components, running side-effects, and integrating with other parts of the code.

To see how Shipyard differs from other ECS implementations, see [Different Types of ECS](./pilgrimage/different-types-of-ecs.md).
