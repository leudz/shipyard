# Add Components

An entity can have any number of components but only one in each storage.  
Adding another component of the same type will replace the existing one.

## World

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity(());

world.add_component(id, (0u32,));
world.add_component(id, (0u32, 1usize));
```

⚠️ We have to use a single element tuple `(T,)` to add a single component.

## View

You'll notice that we use [`EntitiesView`](https://docs.rs/shipyard/0.5.0/shipyard/struct.EntitiesView.html) and not [`EntitiesViewMut`](https://docs.rs/shipyard/0.5.0/shipyard/struct.EntitiesViewMut.html) to add components.  
The entities storage is only used to check if the [`EntityId`](https://docs.rs/shipyard/0.5.0/shipyard/struct.EntityId.html) is alive.  
We could of course use [`EntitiesViewMut`](https://docs.rs/shipyard/0.5.0/shipyard/struct.EntitiesViewMut.html), but exclusive access is not necessary.

If you don't need or want to check if the entity is alive, you can use the [`AddComponent::add_component_unchecked`](https://docs.rs/shipyard/0.5.0/shipyard/trait.AddComponent.html).

```rust, noplaypen
let world = World::new();

let id = world
    .borrow::<EntitiesViewMut>()
    .unwrap()
    .add_entity((), ());

let (entities, mut u32s, mut usizes) = world
    .borrow::<(EntitiesView, ViewMut<u32>, ViewMut<usize>)>()
    .unwrap();

entities.add_component(id, &mut u32s, 0);
entities.add_component(id, (&mut u32s, &mut usizes), (0, 1));
u32s.add_component_unchecked(id, 0);
```
