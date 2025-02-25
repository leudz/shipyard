# Welcome to Shipyard!

[Shipyard](https://github.com/leudz/shipyard) is an Entity Component System focused on usability and flexibility.

ECS is a great way to organize logic and data:

1. Elegant approach for humans
    - Composition over inheritance
    - Separation of concerns
    - Less burdened by lifetimes
2. Optimal design for computers
    - Spatial locality
    - Less pointer chasing

However, programming with an ECS requires thinking about data and logic in a different way than you might be used to.

## How does it work?

Components hold data. Entities are a group of components identified by an Id.

Systems do the heavy lifting: updating components, running side-effects, and integrating with other parts of the code.

# Learn

You can either build a small game with shipyard's concepts showed as you encounter them.\
Or go through a more packed format.

[I want to build a small game](./learn-by-example.md)\
[I prefer a packed format](./fundamentals.md)