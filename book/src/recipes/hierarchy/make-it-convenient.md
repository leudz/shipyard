# Let's make it convenient

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

Since we need access to `EntitiesViewMut` as well as our hierarchy component storages, we implement the `Hierarchy` trait for the type `(EntitiesViewMut<'a>, ViewMut<'a, Parent>, ViewMut<'a, Child>)`.

```rust, noplaypen
fn detach(&mut self, id: EntityId) {
    // remove the Child component - if nonexistent, do nothing (return)
    let child = match Remove::<(Child,)>::remove(&mut self.2, id) {
        (Some(c),) => c,
        _ => return,
    };

    // retrieve and update Parent component from ancestor
    let parent = (&mut self.1).get(child.parent).unwrap();
    parent.num_children -= 1;

    if parent.num_children == 0 {
        // if the number of children is zero, the Parent component must be removed
        Remove::<(Parent,)>::remove(&mut self.1, child.parent)
            .0
            .unwrap();
    } else {
        // the ancestor still has children, and we have to change some linking
        // check if we have to change first_child
        if parent.first_child == id {
            parent.first_child = child.next;
        }
        // remove the detached child from the sibling chain
        (&mut self.2).get(child.prev).unwrap().next = child.next;
        (&mut self.2).get(child.next).unwrap().prev = child.prev;
    }
}
```

Before we move on to `attach`, let's make some observations.

Remember that `self.1` is of type `ViewMut<Parent>`, a mutable View into the `Parent` component storage; `self.2` is a `ViewMut<Child>`. Calling `get` on these with a given `EntityId` returns an `Option<&Parent>` or `Option<&Child>` - the entity may have that component or not.

As the calls to `unwrap` indicate, something might go wrong here.

We don't have to worry as long as we only use the methods in our `Hierarchy` trait.

If you accidentally delete hierarchy components in other places without changing the linking, things will go fatally wrong. This should be seen as a bug and thus panicking with `unwrap` or, even better, `except` is fine.

`attach` looks like this:

```rust, noplaypen
fn attach(&mut self, id: EntityId, parent: EntityId) {
    // the entity we want to attach might already be attached to another parent
    self.detach(id);

    // either the designated parent already has a Parent component – and thus one or more children
    if let Some(p) = (&mut self.1).get(parent) {
        // increase the parent's children counter
        p.num_children += 1;

        // get the ids of the new previous and next siblings of our new child
        let prev = self.2.get(p.first_child).unwrap().prev;
        let next = p.first_child;

        // change the linking
        (&mut self.2).get(prev).unwrap().next = id;
        (&mut self.2).get(next).unwrap().prev = id;

        // add the Child component to the new entity
        self.0
            .add_component(&mut self.2, Child { parent, prev, next }, id);
    } else {
        // in this case our designated parent is missing a Parent component
        // we don't need to change any links, just insert both components
        self.0.add_component(
            &mut self.2,
            Child {
                parent,
                prev: id,
                next: id,
            },
            id,
        );
        self.0.add_component(
            &mut self.1,
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
let world = World::new::<(Parent, Child, usize)>();

world.run::<(EntitiesMut, &mut Parent, &mut Child), _, _>(|mut views| {
    let root1 = views.0.add_entity((), ());
    let root2 = views.0.add_entity((), ());

    let e1 = views.attach_new(root1);
    let e2 = views.attach_new(e1);
    let e3 = views.attach_new(e1);
    let e4 = views.attach_new(e3);

    views.attach(e3, root2);
});
```
