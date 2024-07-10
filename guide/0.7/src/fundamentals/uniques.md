# Unique

Unique components (a.k.a. resources) are useful when you know there will only ever be a single instance of some component.  
In that case there is no need to attach the component to an entity. It also works well as global data without most of its drawback.

As opposed to the default storage uniques are declared using the [`Unique`](https://docs.rs/shipyard/latest/shipyard/trait.Unique.html) trait.

```rust, noplaypen
// Using a derive macro
#[derive(Unique)]
struct Camera;

// By manually implementing the trait
struct Camera;
impl Unique for Camera {}
```

They also need to be initialized with [`add_unique`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.add_unique). We can then access them with [`UniqueView`](https://docs.rs/shipyard/latest/shipyard/struct.UniqueView.html) and [`UniqueViewMut`](https://docs.rs/shipyard/latest/shipyard/struct.UniqueViewMut.html).\

```rust, noplaypen
let world = World::new();

world.add_unique(Camera::new());

world
    .run(|camera: UniqueView<Camera>| {
        // -- snip --
    });
```
