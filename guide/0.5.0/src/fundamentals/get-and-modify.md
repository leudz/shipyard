# Get and Modify Components

To access or update components you can use [`Get::get`](https://docs.rs/shipyard/0.5.0/shipyard/trait.Get.html#tymethod.get). It'll work with both shared and exclusive views.

```rust, noplaypen
let mut world = World::new();

let id = world.add_entity((0u32, 1usize));

let (mut u32s, mut usizes) = world.borrow::<(ViewMut<u32>, ViewMut<usize>)>().unwrap();

*(&mut usizes).get(id).unwrap() += 1;

let (mut i, j) = (&mut u32s, &usizes).get(id).unwrap();
*i += *j as u32;

u32s[id] += 1;
```

When using a single view, if you are certain an entity has the desired component, you can access it via index.

### Fast Get

Using [`get`](https://docs.rs/shipyard/0.5.0/shipyard/trait.Get.html#tymethod.get) with [`&mut ViewMut<T>`](https://docs.rs/shipyard/0.5.0/shipyard/struct.ViewMut.html) will return [`Mut<T>`](https://docs.rs/shipyard/0.5.0/shipyard/struct.Mut.html). This struct helps fine track component modification.  
[`FastGet::fast_get`](https://docs.rs/shipyard/0.5.0/shipyard/trait.FastGet.html#tymethod.fast_get) can be used to opt out of this fine tracking and get back `&mut T`.
