# Intro to ECS

ECS stands for Entity Component System. It's a great way to organize logic and data with an emphasis on usability and speed.

There are two main benefits to using an ECS:

1. Elegant approach for humans
    - Composition over inheritance
    - Separation of concerns
    - Array of Structures
2. Optimal design for computers.
    - Structure of Arrays
    - Data alignment
    - Less pointer chasing

However, programming with an ECS requires thinking about data and logic in a different way than you might be used to. Also, the optimization techniques need explicit choice and tuning to get the greatest benefit (see [Optimization with Packs](./optimization-with-packs.md))

# How does it work?

Entities are just newtype wrappers around a `u64`. Under the hood they are really nothing other than an index.

Components hold data. Only data. No logic. They _can_ of course contain methods but the ECS only sees the hard data.

Systems do the heavy lifting: updating components, running side-effects, and integrating with other parts of the code.

(For more details including how Shipyard differs from other ECS implementations, see [Under the Hood](../under-the-hood.md))