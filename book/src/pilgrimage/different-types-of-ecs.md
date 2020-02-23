# Different Types of ECS

ECS is a pattern and there's no right or wrong way to implement it, even the pattern doesn't have a clear definition.  
Shipyard is a `SparseSet` based ECS, so is [EnTT](https://github.com/skypjack/entt) but there are multiple other implementations, here's a few ones:

### Archetypes

Let's start with the most popular one, thanks to Unity.  
This implementation groups entities and their components based on which one they have. For example all entities with a `usize` and `u32` components will be stored together in memory.  
When an entity gains or loses a component it moves from an archetype to another.  
[Unity's dots](https://unity.com/dots), [Flecs](https://github.com/SanderMertens/flecs/) or [Legion](https://github.com/TomGillen/legion) are implemented using archetypes.

### Bitset

This is how [Specs](https://github.com/amethyst/specs) is implemented.  
Each component has a storage and a bitset, when an entity has a component the bit at its index is set.  
Iterating becomes a bitsets iteration, we `or` them which yields the entities with all required components.

### Option

A simple ECS implementation can be made with `Option`, each component has a `Vec<Option<T>>` storage.  
The components default to `None`, when we add a component it switches to `Some` at the entity's index.  
When iterating we simply have to go through all requested storages and check if the components are `Some`.
