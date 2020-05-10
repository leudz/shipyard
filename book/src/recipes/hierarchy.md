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

## Let's make it convenient

We begin with two useful methods in a trait declaration:

```rust, noplaypen
trait Hierarchy {
    // Removes the child status of an entity.
    fn detach(&mut self, id: EntityId);

    // Attaches an entity as a child to a given parent entity.
    fn attach(&mut self, id: EntityId, parent: EntityId);
}
```

With these, you'll be able to not only insert new entities into the tree but also move a whole subtree – a child with all its descendants – to another parent.

Since we need access to `EntitiesViewMut` as well as our hierarchy component storages, we implement the `Hierarchy` trait for the type `(EntitiesViewMut<'_>, ViewMut<'_, Parent>, ViewMut<'_, Child>)`.

```rust, noplaypen
fn detach(&mut self, id: EntityId) {
    let (_, parents, children) = self;
    // remove the Child component - if nonexistent, do nothing
    if let Some(OldComponent::Owned(child)) = children.remove(id) {
        // retrieve and update Parent component from ancestor
        let parent = &mut parents[child.parent];
        parent.num_children -= 1;

        if parent.num_children == 0 {
            // if the number of children is zero, the Parent component must be removed
            parents.remove(child.parent);
        } else {
            // the ancestor still has children, and we have to change some linking
            // check if we have to change first_child
            if parent.first_child == id {
                parent.first_child = child.next;
            }
            // remove the detached child from the sibling chain
            children[child.prev].next = child.next;
            children[child.next].prev = child.prev;
        }
    }
}
```

Before we move on to `attach`, let's make some observations.

We use indexing on `parents` and `children` but if the entity doesn't have the component it'll `unwrap`.

We don't have to worry as long as we only use the methods in our `Hierarchy` trait.

If you accidentally delete hierarchy components in other places without changing the linking, things will go fatally wrong. If you want to catch these errors you might want to use `get` and handle the error (for example with `expect`).

`attach` looks like this:

```rust, noplaypen
fn attach(&mut self, id: EntityId, parent: EntityId) {
    // the entity we want to attach might already be attached to another parent
    self.detach(id);

    let (entities, parents, children) = self;

    // either the designated parent already has a Parent component – and thus one or more children
    if let Ok(p) = parents.get(parent) {
        // increase the parent's children counter
        p.num_children += 1;

        // get the ids of the new previous and next siblings of our new child
        let prev = children[p.first_child].prev;
        let next = p.first_child;

        // change the linking
        children[prev].next = id;
        children[next].prev = id;

        // add the Child component to the new entity
        entities.add_component(children, Child { parent, prev, next }, id);
    } else {
        // in this case our designated parent is missing a Parent component
        // we don't need to change any links, just insert both components
        entities.add_component(
            children,
            Child {
                parent,
                prev: id,
                next: id,
            },
            id,
        );
        entities.add_component(
            parents,
            Parent {
                num_children: 1,
                first_child: id,
            },
            parent,
        );
    }
}
```

We can now add another handy method to our trait:

```rust, noplaypen
// Creates a new entity and attaches it to the given parent.
fn attach_new(&mut self, parent: EntityId) -> EntityId;`
```

```rust, noplaypen
fn attach_new(&mut self, parent: EntityId) -> EntityId {
    let id = self.0.add_entity((), ());
    self.attach(id, parent);
    id
}
```

And lastly a simple usage example:

```rust, noplaypen
let world = World::new();

let mut hierarchy = world.borrow::<(EntitiesViewMut, ViewMut<Parent>, ViewMut<Child>)>();

let root1 = hierarchy.0.add_entity((), ());
let root2 = hierarchy.0.add_entity((), ());

let e1 = hierarchy.attach_new(root1);
let _e2 = hierarchy.attach_new(e1);
let e3 = hierarchy.attach_new(e1);
let _e4 = hierarchy.attach_new(e3);

hierarchy.attach(e3, root2);
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
struct ChildrenIter<C> {
    get_child: C,
    cursor: (EntityId, usize),
}

impl<'a, C> Iterator for ChildrenIter<C>
where
    C: Get<Out = &'a Child> + Copy,
{
    type Item = EntityId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
            let ret = self.cursor.0;
            self.cursor.0 = self.get_child.get(self.cursor.0).unwrap().next;
            Some(ret)
        } else {
            None
        }
    }
}
```

Note that we don't implement `Iterator` for `ViewMut<Child>` directly, but for a type that implements the `GetComponent` trait. This way, our iterator can be used with `View` as well as `ViewMut`.

The next one is the `AncestorIter`:

```rust, noplaypen
struct AncestorIter<C> {
    get_child: C,
    cursor: EntityId,
}

impl<'a, C> Iterator for AncestorIter<C>
where
    C: Get<Out = &'a Child> + Copy,
{
    type Item = EntityId;

    fn next(&mut self) -> Option<Self::Item> {
        self.get_child.get(self.cursor).ok().map(|child| {
            self.cursor = child.parent;
            child.parent
        })
    }
}
```

Easy.

`DescendantIter` will be a bit more complicated. We choose to implement a depth-first variant using recursion.

It is based on the code for the `ChildrenIter` but comes with an additional stack to keep track of the current level the cursor is in:
- Push a new level to the stack if we encounter a `Parent` component.
- Pop the last level from the stack whenever we run out of siblings, then carry on where we left off.

```rust, noplaypen
struct DescendantsIter<P, C> {
    get_parent: P,
    get_child: C,
    cursors: Vec<(EntityId, usize)>,
}

impl<'a, P, C> Iterator for DescendantsIter<P, C>
where
    P: Get<Out = &'a Parent> + Copy,
    C: Get<Out = &'a Child> + Copy,
{
    type Item = EntityId;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cursor) = self.cursors.last_mut() {
            if cursor.1 > 0 {
                cursor.1 -= 1;
                let ret = cursor.0;
                cursor.0 = self.get_child.get(cursor.0).unwrap().next;
                if let Ok(parent) = self.get_parent.get(ret) {
                    self.cursors.push((parent.first_child, parent.num_children));
                }
                Some(ret)
            } else {
                self.cursors.pop();
                self.next()
            }
        } else {
            None
        }
    }
}
```

What we still need to do is to implement a simple trait with methods that return nicely initialized `*Iter` structs for us:

```rust, noplaypen
trait HierarchyIter<'a, P, C> {
    fn ancestors(&self, id: EntityId) -> AncestorIter<C>;
    fn children(&self, id: EntityId) -> ChildrenIter<C>;
    fn descendants(&self, id: EntityId) -> DescendantsIter<P, C>;
}

impl<'a, P, C> HierarchyIter<'a, P, C> for (P, C)
where
    P: Get<Out = &'a Parent> + Copy,
    C: Get<Out = &'a Child> + Copy,
{
    fn ancestors(&self, id: EntityId) -> AncestorIter<C> {
        let (_, children) = self;

        AncestorIter {
            get_child: *children,
            cursor: id,
        }
    }

    fn children(&self, id: EntityId) -> ChildrenIter<C> {
        let (parents, children) = self;

        ChildrenIter {
            get_child: *children,
            cursor: parents
                .get(id)
                .map_or((id, 0), |parent| (parent.first_child, parent.num_children)),
        }
    }

    fn descendants(&self, id: EntityId) -> DescendantsIter<P, C> {
        let (parents, children) = self;

        DescendantsIter {
            get_parent: *parents,
            get_child: *children,
            cursors: parents.get(id).map_or_else(
                |_| Vec::new(),
                |parent| vec![(parent.first_child, parent.num_children)],
            ),
        }
    }
}
```

Cool. Let's extend the former usage example into a little test.

```rust, noplaypen
#[test]
fn test_hierarchy() {
    let world = World::new();

    let mut hierarchy = world.borrow::<(EntitiesViewMut, ViewMut<Parent>, ViewMut<Child>)>();

    let root1 = hierarchy.0.add_entity((), ());
    let root2 = hierarchy.0.add_entity((), ());

    let e1 = hierarchy.attach_new(root1);
    let e2 = hierarchy.attach_new(e1);
    let e3 = hierarchy.attach_new(e1);
    let e4 = hierarchy.attach_new(e3);

    hierarchy.attach(e3, root2);

    let e5 = hierarchy.attach_new(e3);

    assert!((&hierarchy.1, &hierarchy.2)
        .children(e3)
        .eq([e4, e5].iter().cloned()));

    assert!((&hierarchy.1, &hierarchy.2)
        .ancestors(e4)
        .eq([e3, root2].iter().cloned()));

    assert!((&hierarchy.1, &hierarchy.2)
        .descendants(root1)
        .eq([e1, e2].iter().cloned()));

    assert!((&hierarchy.1, &hierarchy.2)
        .descendants(root2)
        .eq([e3, e4, e5].iter().cloned()));
}
```

## Removing entities from the hierarchy

Removing an entity from the hierarchy means removing its `Parent` and `Child` components.

To remove an entity's `Child` component, we can simply reuse `detach`. Removing its `Parent` component must be done with caution. This entity's children now become orphans – we have to detach them as well.

Both methods can be added to our `Hierarchy` trait:

```rust, noplaypen
fn remove(&mut self, id: EntityId) {
    self.detach(id);

    let children = (&self.1, &self.2).children(id).collect::<Vec<_>>();
    for child_id in children {
        self.detach(child_id);
    }
    self.1.remove(id);
}
```

A method that removes a whole subtree is easy to write by making use of recursion again:

```rust, noplaypen
fn remove_all(&mut self, id: EntityId) {
    let (_, parents, children) = self;

    for child_id in (&*parents, &*children).children(id).collect::<Vec<_>>() {
        self.remove_all(child_id);
    }
    self.remove(id);
}
```

That's it! We can now add the following code to the end of our test from the last chapter:

```rust, noplaypen
hierarchy.detach(e1);

assert!((&hierarchy.1, &hierarchy.2).descendants(root1).eq(None));
assert!((&hierarchy.1, &hierarchy.2).ancestors(e1).eq(None));
assert!((&hierarchy.1, &hierarchy.2).children(e1).eq([e2].iter().cloned()));

hierarchy.remove(e1);

assert!((&hierarchy.1, &hierarchy.2).children(e1).eq(None));

hierarchy.remove_all(root2);

assert!((&hierarchy.1, &hierarchy.2).descendants(root2).eq(None));
assert!((&hierarchy.1, &hierarchy.2).descendants(e3).eq(None));
assert!((&hierarchy.1, &hierarchy.2).ancestors(e5).eq(None));
```

## Sorting

The order between siblings may or may not play a role in your project.

However, a simple sorting for children can be done in two steps:

- Collect all children into a `Vec` and sort it.
- Adjust the linking in the `Child` components according to the sorted list.

We can add this method to the `Hierarchy` trait:

```rust, noplaypen
fn sort_children_by<F>(&mut self, id: EntityId, compare: F)
where
    F: FnMut(&EntityId, &EntityId) -> std::cmp::Ordering,
{
    let (_, parents, children_storage) = self;

    let mut children = (&*parents, &*children_storage)
        .children(id)
        .collect::<Vec<EntityId>>();
    if children.len() > 1 {
        children.sort_by(compare);
        // set first_child in Parent component
        parents[id].first_child = children[0];
        // loop through children and relink them
        for i in 0..children.len() - 1 {
            children_storage[children[i]].next = children[i + 1];
            children_storage[children[i + 1]].prev = children[i];
        }
        children_storage[children[0]].prev = *children.last().unwrap();
        children_storage[*children.last().unwrap()].next = children[0];
    }
}
```

Again a small test demonstrates the usage:

```rust, noplaypen
#[test]
fn test_sorting() {
    let world = World::new();

    let (mut hierarchy, mut usizes) = world.borrow::<(
        (EntitiesViewMut, ViewMut<Parent>, ViewMut<Child>),
        ViewMut<usize>,
    )>();
    
    let root = hierarchy.0.add_entity((), ());

    let e0 = hierarchy.attach_new(root);
    let e1 = hierarchy.attach_new(root);
    let e2 = hierarchy.attach_new(root);
    let e3 = hierarchy.attach_new(root);
    let e4 = hierarchy.attach_new(root);

    hierarchy.0.add_component(&mut usizes, 7, e0);
    hierarchy.0.add_component(&mut usizes, 5, e1);
    hierarchy.0.add_component(&mut usizes, 6, e2);
    hierarchy.0.add_component(&mut usizes, 1, e3);
    hierarchy.0.add_component(&mut usizes, 3, e4);

    assert!((&hierarchy.1, &hierarchy.2)
        .children(root)
        .eq([e0, e1, e2, e3, e4].iter().cloned()));

    hierarchy.sort_children_by(root, |a, b| usizes[*a].cmp(&usizes[*b]));

    assert!((&hierarchy.1, &hierarchy.2)
        .children(root)
        .eq([e3, e4, e1, e2, e0].iter().cloned()));
}
```

## Do it yourself!

We recommend that you build your own hierarchy system fitted to your specific needs. In deviation of the above code examples you may want:

- a single hierarchy component instead of two,
- breadth-first instead of depth-first traversal,
- different sorting methods,
- etc.

## Further reading

These notes are based on ideas presented in a highly recommended article by skypjack: [ECS back and forth](https://skypjack.github.io/2019-06-25-ecs-baf-part-4/).
