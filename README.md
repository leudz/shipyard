# Shipyard

Shipyard is an Entity Component System. While usable it is far from finished.

[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![LICENSE](https://img.shields.io/badge/license-apache-blue.svg)](LICENSE-APACHE)
[![Crates.io](https://img.shields.io/crates/v/shipyard.svg)](https://crates.io/crates/shipyard)
[![Documentation](https://docs.rs/shipyard/badge.svg)](https://docs.rs/shipyard)
[![User's Guide](https://img.shields.io/badge/user's%20guide-current-blueviolet)](https://leudz.github.io/shipyard/)
[![Chat](https://img.shields.io/badge/zulip-join_chat-brightgreen.svg)](https://shipyard.zulipchat.com/join/zrakw74eyqongdul9bib769w/)

## Interesting features
- **Packing** can enable perfect components alignment, allowing fast iteration
but also SIMD instructions. To learn how it's done read Michele **skypjack** Caini's
[great blog article](https://skypjack.github.io/2019-03-21-ecs-baf-part-2-insights/).
- **Automatic scheduling** you just have to tell which systems you want to run and
the `World` will do the rest.

## Simple Example
```rust
use shipyard::prelude::*;

struct Health(f32);
struct Position { x: f32, y: f32 };

#[system(InAcid)]
fn run(pos: &Position, health: &mut Health) {
    (&pos, &mut health).iter().for_each(|(pos, mut health)| {
        if is_in_acid(pos) {
            health.0 -= 1.0;
        }
    });
}

fn is_in_acid(pos: &Position) -> bool {
    // it's wet season

    true
}

let world = World::new::<(Position, Health)>();

world.run::<(EntitiesMut, &mut Position, &mut Health), _, _>(|mut entities, mut pos, mut health| {
    entities.add_entity((&mut pos, &mut health), Position { x: 0.0, y: 0.0 }, Health(1000.0));
});

world.add_workload("In acid", InAcid);
world.run_default();
```

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