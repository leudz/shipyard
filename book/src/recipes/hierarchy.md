# Building an Entity Hierarchy with Shipyard

Hierarchies are a very commonly used organizational structure in game development. An important example is a transform hierarchy: child entities move along with their parents.

How can we build such a hierarchy of entities in shipyard?

One method is to use a secondary data structure which represents the hierarchy.

But an ECS already has all the means to store data: components. So let's use them!

Below you won't find a ready-to-use solution, rather some hints on how to start with your own hierarchy implementation, tailored to your requirements.

## Parents and Children

Think about the different roles an entity can take in a hierarchy. It can be:

- a parent (root node),
- a parent and a child (intermediate node),
- a child (leaf node).

From this we can derive two simple, composable component types:

A `Parent` component stores the number of its children and the first child:

```rust, noplaypen
struct Parent {
    num_children: usize,
    first_child: EntityId,
}
```

A `Child` component links to its parent as well as neighbor siblings:

```rust, noplaypen
struct Child {
    parent: EntityId,
    prev: EntityId,
    next: EntityId,
}
```

As you can see, we simply store `EntityId`s to refer to other entities inside a component.

Note that `Option`s are completely avoided by making the sibling chain circular:

- Last child's `next` points to the first child.
- First child's `prev` points to the last child.

Our entire hierarchy structure resides only in `Parent` and `Child` components – nice!

But it'd be a hassle to create them manually each time you want to insert an entity into the tree.
