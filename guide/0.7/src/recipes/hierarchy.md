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
{{#include ../../../../tests/book/hierarchy.rs:parent}}
```

A `Child` component links to its parent as well as neighbor siblings:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:child}}
```

As you can see, we simply store `EntityId`s to refer to other entities inside a component.

Note that `Option`s are completely avoided by making the sibling chain circular:

- Last child's `next` points to the first child.
- First child's `prev` points to the last child.

Our entire hierarchy structure resides only in `Parent` and `Child` components – nice!

But it'd be a hassle to create them manually each time you want to insert an entity into the tree.

## Let's make it convenient

We begin with two useful methods in a trait declaration:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:hierarchy_partial}}
```

With these, you'll be able to not only insert new entities into the tree but also move a whole subtree – a child with all its descendants – to another parent.

Since we need access to `EntitiesViewMut` as well as our hierarchy component storages, we implement the `Hierarchy` trait for the type `(EntitiesViewMut<'_>, ViewMut<'_, Parent>, ViewMut<'_, Child>)`.

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:detach}}
```

Before we move on to `attach`, let's make some observations.

We use indexing on `parents` and `children` but if the entity doesn't have the component it'll `unwrap`.

We don't have to worry as long as we only use the methods in our `Hierarchy` trait.

If you accidentally delete hierarchy components in other places without changing the linking, things will go fatally wrong. If you want to catch these errors you might want to use `get` and handle the error (for example with `expect`).

`attach` looks like this:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:attach}}
```

We can now add another handy method to our trait:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:attach_new}}
```

And lastly a simple usage example:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:basic}}
```

## Traversing the hierarchy

There are different ways the hierarchy can be queried.

For example, we may want to know the parent of a given entity. Doing this is simply done by inspecting its child component - if there is one.

However, sometimes you might need

- all children,
- all ancestors,
- or all descendants of a given entity.

A perfect use case for iterators! An iterator has to implement the `next` method from the `Iterator` trait.

We start with a `ChildrenIter`, which is pretty straightforward:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:children_iter}}
```

Note that we don't implement `Iterator` for `ViewMut<Child>` directly, but for a type that implements the `GetComponent` trait. This way, our iterator can be used with `View` as well as `ViewMut`.

The next one is the `AncestorIter`:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:ancestor_iter}}
```

Easy.

`DescendantIter` will be a bit more complicated. We choose to implement a depth-first variant using recursion.

It is based on the code for the `ChildrenIter` but comes with an additional stack to keep track of the current level the cursor is in:
- Push a new level to the stack if we encounter a `Parent` component.
- Pop the last level from the stack whenever we run out of siblings, then carry on where we left off.

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:descendants_iter}}
```

What we still need to do is to implement a simple trait with methods that return nicely initialized `*Iter` structs for us:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:hierarchy_iter}}
```

Cool. Let's extend the former usage example into a little test.

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:test_hierarchy}}
{{#include ../../../../tests/book/hierarchy.rs:bracket}}
```

## Removing entities from the hierarchy

Removing an entity from the hierarchy means removing its `Parent` and `Child` components.

To remove an entity's `Child` component, we can simply reuse `detach`. Removing its `Parent` component must be done with caution. This entity's children now become orphans – we have to detach them as well.

Both methods can be added to our `Hierarchy` trait:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:remove}}
```

A method that removes a whole subtree is easy to write by making use of recursion again:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:remove_all}}
```

That's it! We can now add the following code to the end of our test from the last chapter:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:test_hierarchy_detach}}
```

## Sorting

The order between siblings may or may not play a role in your project.

However, a simple sorting for children can be done in two steps:

- Collect all children into a `Vec` and sort it.
- Adjust the linking in the `Child` components according to the sorted list.

We can add this method to the `Hierarchy` trait:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:sort}}
```

Again a small test demonstrates the usage:

```rust, noplaypen
{{#include ../../../../tests/book/hierarchy.rs:test_sorting}}
```

## Do it yourself!

We recommend that you build your own hierarchy system fitted to your specific needs. In deviation of the above code examples you may want:

- a single hierarchy component instead of two,
- breadth-first instead of depth-first traversal,
- different sorting methods,
- etc.

## Further reading

These notes are based on ideas presented in a highly recommended article by skypjack: [ECS back and forth](https://skypjack.github.io/2019-06-25-ecs-baf-part-4/).
