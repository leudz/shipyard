# Shipyard

Shipyard is an Entity Component System focused on usability and speed.

[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![LICENSE](https://img.shields.io/badge/license-apache-blue.svg)](LICENSE-APACHE)
[![Crates.io](https://img.shields.io/crates/v/shipyard.svg)](https://crates.io/crates/shipyard)
[![Documentation](https://docs.rs/shipyard/badge.svg)](https://docs.rs/shipyard)
[![User's Guide](https://img.shields.io/badge/user's%20guide-current-blueviolet)](https://leudz.github.io/shipyard/book)
[![Demo](https://img.shields.io/badge/demo-launch-yellow)](https://leudz.github.io/shipyard/demo)
[![Chat](https://img.shields.io/badge/zulip-join_chat-brightgreen.svg)](https://shipyard.zulipchat.com/join/zrakw74eyqongdul9bib769w/)

While usable it is far from finished, there's a lot of planned features, nearly all being backward compatible additions.

Most discussions about current and future features happen on zulip, feel free to join in to follow the development, ask any question or just say hi.

If you are new here, the [user guide](https://leudz.github.io/shipyard/book) is a great place to learn all about Shipyard!

## Simple Example
```rust
use shipyard::prelude::*;

struct Health(f32);
struct Position { x: f32, y: f32 }

#[system(InAcid)]
fn run(pos: &Position, mut health: &mut Health) {
    (&pos, &mut health).iter()
        .filter(|(pos, _)| is_in_acid(pos))
        .for_each(|(pos, mut health)| {
            health.0 -= 1.0;
        });
}

fn is_in_acid(pos: &Position) -> bool {
    // it's wet season
    true
}

let world = World::new();

{
    let (mut entities, mut positions, mut healths) =
        world.borrow::<(EntitiesMut, &mut Position, &mut Health)>();
   
    entities.add_entity(
        (&mut positions, &mut healths),
        (Position { x: 0.0, y: 0.0 },
        Health(1000.0))
    );
}

world.run_system::<InAcid>();
```

## Past, Present and Future

I initially started to make an ECS to learn how it works. After a failed attempt and learning a lot from it and other ECS out there, I started to work on Shipyard.

[Specs](https://github.com/amethyst/specs) was already well established as the go-to Rust ECS but I thought I could do better and went with [EnTT](https://github.com/skypjack/entt) core data-structure: `SparseSet`.

It turned out to be extremely flexible and is still the core of Shipyard. You can pay for what you want: iteration speed, memory, ease of use,...

And it allowed amazing features:
- No component boilerplate
- Very simple systems
- Powerful inner and outer system parallelism
- Ability to add/remove components while adding/removing entities
- Chunk iteration
- And a lot more!

Today I wouldn't say Shipyard is better or worse than Specs, it's just different. I'm really happy with it and the future looks very promising, especially:
- [Pipeline](https://github.com/leudz/shipyard/issues/44)
- [Events](https://github.com/leudz/shipyard/issues/22)
- [Nested packs](https://github.com/leudz/shipyard/issues/47)
- [Shared components](https://github.com/leudz/shipyard/issues/38)
- [Iterator blueprint](https://github.com/leudz/shipyard/issues/41)

## Similar Projects

- [EnTT](https://github.com/skypjack/entt) - C++ library built on `SparseSet` and providing grouping functionality, a lot of its designs are explained in [a blog](https://skypjack.github.io/). This is where Shipyard's `SparseSet` and most packs come from
- [Specs](https://github.com/amethyst/specs) - Rust library relying on `BitSet` and allowing to use multiple storage types
- [Legion](https://github.com/TomGillen/legion) - Rust library based on archetypes
- [Hecs](https://github.com/Ralith/hecs) - Rust library also based on archetypes but keeping a minimalistic approach

## Performance

If you're wondering how fast Shipyard is you can look at a few graphs in [this issue](https://github.com/leudz/shipyard/issues/61).  
There is still a lot of room for optimization, the current focus is more on adding functionalities.

## Features

- **parallel** *(default)* &mdash; adds parallel iterators and dispatch
- **proc** *(default)* &mdash; adds `system` proc macro
- **serde** &mdash; adds (de)serialization support with [serde](https://github.com/serde-rs/serde)
- **non_send** &mdash; add methods and types required to work with `!Send` components
- **non_sync** &mdash; add methods and types required to work with `!Sync` components
- **std** *(default)* &mdash; let shipyard use the standard library

## Unsafe

This crate uses `unsafe` both because sometimes there's no way around it, and for performance gain.  
Releases should have all invocation of `unsafe` explained.  
If you find places where a safe alternative is possible without repercussion (small ones are sometimes acceptable) feel free to open an issue or a PR.

## Origin of the Name

Assembly lines take input, process it at each step, and output a result.  You can have multiple lines working in parallel as long as they don't bother each other.

Shipyards such as the [Venetian Arsenal](https://en.wikipedia.org/wiki/Venetian_Arsenal) are some of the oldest examples of successful, large-scale, industrial assembly lines.  So successful that it could output a fully-finished ship _every day_.

*Shipyard* is a project you can use to build your own highly-parallel software processes.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
