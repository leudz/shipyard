# Shipyard <!-- omit in toc -->

Shipyard is an Entity Component System focused on usability and speed.

[![LICENSE](https://img.shields.io/crates/l/shipyard)](LICENSE-APACHE)
[![Crates.io](https://img.shields.io/crates/v/shipyard)](https://crates.io/crates/shipyard)
[![Documentation](https://docs.rs/shipyard/badge.svg)](https://docs.rs/shipyard)
[![Chat](https://img.shields.io/badge/zulip-join_chat-brightgreen.svg)](https://shipyard.zulipchat.com)

If you have any question or want to follow the development more closely join the [Zulip](https://shipyard.zulipchat.com).

There's two big learning resources:
- (Soon™) The Tutorial for people new to ECS or who prefer to learn by making things.
- [The Guide](https://leudz.github.io/shipyard/guide) for people that already know how to use an ECS and mostly want to learn Shipyard's syntax.  
  It also goes into greater depth and provides useful recipes for common design patterns.

## Simple Example <!-- omit in toc -->
```rust
use shipyard::*;

struct Health(f32);
struct Position {
    _x: f32,
    _y: f32,
}

fn in_acid(positions: View<Position>, mut healths: ViewMut<Health>) {
    for (_, mut health) in (&positions, &mut healths)
        .iter()
        .filter(|(pos, _)| is_in_acid(pos))
    {
        health.0 -= 1.0;
    }
}

fn is_in_acid(_: &Position) -> bool {
    // it's wet season
    true
}

fn main() {
    let world = World::new();

    world.run(
        |mut entities: EntitiesViewMut,
         mut positions: ViewMut<Position>,
         mut healths: ViewMut<Health>| {
            entities.add_entity(
                (&mut positions, &mut healths),
                (Position { _x: 0.0, _y: 0.0 }, Health(1000.0)),
            );
        },
    );

    world.run(in_acid);
}
```

## Table of Contents <!-- omit in toc -->
- [Let there be SparseSets](#let-there-be-sparsesets)
- [Systems](#systems)
  - [Not just storage](#not-just-storage)
  - [Return](#return)
  - [Generics](#generics)
  - [All at once](#all-at-once)
- [Unique Storage (Resource)](#unique-storage-resource)
- [!Send and !Sync Components](#send-and-sync-components)
- [Workload](#workload)
- [Cargo Features](#cargo-features)
- [Unsafe](#unsafe)
- [License](#license)
- [Contributing](#contributing)

## Let there be SparseSets

I initially started to make an ECS to learn how they work. After a failed attempt and some research, I started to work on Shipyard.

[Specs](https://github.com/amethyst/specs) was already well established as the go-to Rust ECS but I thought I could do better and went with [EnTT](https://github.com/skypjack/entt) core data-structure: `SparseSet`.

It's extremely flexible and is the core data structure behind Shipyard.  
I wouldn't say Shipyard is better or worse than Specs, it's just different.

## Systems

Systems make it very easy to split your logic in manageable chunks. Shipyard takes the concept quite far.

You always start with a function or closure and almost always take a few views (reference to storage) as arguments.  
The basic example shown above does just that:
```rust
fn in_acid(positions: View<Position>, mut healths: ViewMut<Health>) {
    // -- snip --
}
```
A function with two views as argument.

### Not just storage

The first argument doesn't have to be a view, you can pass any data to a system. You don't even have to own it.

```rust
fn in_acid(season: &Season, positions: View<Position>, mut healths: ViewMut<Health>) {
    // -- snip --
}

world.run_with_data(in_acid, &season);
```
You have to provide the data when running the system of course.

### Return

Systems can also have a return type, if run directly with `World::run` or `AllStorages::run` you'll get the returned value right away.  
For workloads you can only get back errors.

```rust
fn lowest_hp(healths: View<Health>) -> EntityId {
    // -- snip --
}

let entity = world.run(lowest_hp);
```

### Generics

Just like any function you can add some generics. You'll have to specify them when running the system.

```rust
fn in_acid<F: Float>(positions: View<Position<F>>, mut healths: ViewMut<Health>) {
    // -- snip --
}

world.run(in_acid::<f32>);
```

### All at once

You can of course use all of them at the same time.

```rust
fn debug<T: Debug + 'static>(fmt: &mut Formatter, view: View<T>) -> Result<(), fmt::Error> {
    // -- snip --
}

world.run_with_data(debug::<u32>, fmt)?;
```

## Unique Storage (Resource)

Unique storages are used to store data you only have once in the `World` and aren't related to any entity.

```rust
fn render(renderer: UniqueView<Renderer>) {
    // -- snip --
}

world.add_unique(Renderer::new());
```

## !Send and !Sync Components

`!Send` and `!Sync` components can be stored directly in the `World` and accessed almost just like any other component.  
Make sure to add the cargo feature to have access to this functionality.

```rust
fn run(rcs: NonSendSync<View<Rc<u32>>>) {
    // -- snip --
}
```

## Workload

Workloads make it easy to run multiple systems again and again. They also automatically schedule systems so you don't have borrow error when trying to use multiple threads.

```rust
fn in_acid(positions: View<Position>, mut healths: ViewMut<Health>) {
    // -- snip --
}

fn tag_dead(entities: EntitiesView, healths: View<Health>, mut deads: ViewMut<Dead>) {
    for (id, health) in healths.iter().with_id() {
        if health.0 == 0.0 {
            entities.add_component(&mut deads, Dead, id);
        }
    }
}

fn remove_dead(mut all_storages: AllStoragesViewMut) {
    all_storages.remove_any::<(Dead,)>();
}

world
    .add_workload("Rain")
    .with_system(system!(in_acid))
    .with_system(system!(tag_dead))
    .with_system(system!(remove_dead))
    .build();

world.run_workload("Rain");
world.run_workload("Rain");
```

The system macro acts as duck tape while waiting for some features in the language, it will disappear as soon as possible.  
You can make workloads without it but I strongly recommended to use it.

## Cargo Features

- **panic** *(default)* adds panicking functions
- **parallel** *(default)* &mdash; adds parallel iterators and dispatch
- **serde** &mdash; adds (de)serialization support with [serde](https://github.com/serde-rs/serde)
- **non_send** &mdash; adds methods and types required to work with `!Send` components
- **non_sync** &mdash; adds methods and types required to work with `!Sync` components
- **std** *(default)* &mdash; lets shipyard use the standard library

## Unsafe

This crate uses `unsafe` both because sometimes there's no way around it, and for performance gain.  
Releases should have all invocation of `unsafe` explained.  
If you find places where a safe alternative is possible without repercussion (small ones are sometimes acceptable) please open an issue or a PR.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
