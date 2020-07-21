# Move it

In the previous chapter we created our character, yes the `@`. But we can't do anything with it, let's change that.

We'll need a few new imports:
```diff
-use rltk::{GameState, Rltk, RltkBuilder, RGB};
```
```rust, noplaypen
use rltk::{GameState, Rltk, RltkBuilder, VirtualKeyCode, RGB};
use std::cmp::{max, min};
```

Next, we'll make a new system:
```rust, noplaypen
fn move_player(ctx: &mut Rltk, mut positions: ViewMut<Position>) {
    let [delta_x, delta_y] = match ctx.key {
        Some(VirtualKeyCode::Left) => [-1, 0],
        Some(VirtualKeyCode::Right) => [1, 0],
        Some(VirtualKeyCode::Up) => [0, -1],
        Some(VirtualKeyCode::Down) => [0, 1],
        _ => return,
    };

    for pos in (&mut positions).iter() {
        pos.x = min(79, max(0, pos.x + delta_x));
        pos.y = min(49, max(0, pos.y + delta_y));
    }
}
```

`Rltk.key` will tell us which key is pressed and we'll move the player accordingly.

This system is a bit different from the previous one since it takes an argument that we don't own and isn't a view.  
For this kind of system we'll have to use [`World::run_with_data`](https://docs.rs/shipyard/latest/shipyard/struct.World.html#method.run_with_data).

We'll run the system after clearing the console and before rendering the player:
```rust, noplaypen
self.ecs.run_with_data(move_player, ctx);
```

If you run your program you should be able to move the player with the arrow keys.

## A wall or two (or 400)

We can move our character but space isn't a dream place for everyone.

To make it more interesting we'll fill the console with tiles:
```rust, noplaypen
#[derive(Clone, Copy)]
enum Tile {
    Floor,
    Wall,
}
```

We'll hold all these tiles in a `[Tile; 4000]` and store it in the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html):
```rust, noplaypen
fn create_map() -> [Tile; 4000] {
    // we start without any wall
    let mut map = [Tile::Floor; 80 * 50];

    // we replace the outer edges with walls
    for x in 0..80 {
        map[index_of(x, 0)] = Tile::Wall;
        map[index_of(x, 49)] = Tile::Wall;
    }
    for y in 1..49 {
        map[index_of(0, y)] = Tile::Wall;
        map[index_of(79, y)] = Tile::Wall;
    }

    let mut rng = rltk::RandomNumberGenerator::new();

    // we randomly place up to 400 walls
    for _ in 0..400 {
        let x = rng.range(0, 80);
        let y = rng.range(0, 50);
        let idx = index_of(x, y);
        // make sure the player isn't in a wall
        if idx != index_of(40, 25) {
            map[idx] = Tile::Wall;
        }
    }

    map
}

/// Returns the index based on coordinates.
fn index_of(x: i32, y: i32) -> usize {
    ((y * 80) + x) as usize
}
```

We could add the map to the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) as an entity but we'll only have one and accessing it would require storing its `EntityId`.  
For exactly this purpose another type of storage exists: unique storage.

Let's add the map right after creating the player, in `main`:
```rust, noplaypen
gs.ecs.add_unique(create_map());
```

With the map created and stored in the [`World`](https://docs.rs/shipyard/latest/shipyard/struct.World.html) there is just one thing left to do: render it.

- imports:
    ```diff
    -use shipyard::{EntitiesViewMut, IntoIter, View, ViewMut, World};
    ```
    ```rust, noplaypen
    use shipyard::{EntitiesViewMut, IntoIter, UniqueView, View, ViewMut, World};
    ```

- new system:
    ```rust, noplaypen
    fn draw_map(ctx: &mut Rltk, map: UniqueView<[Tile; 4000]>) {
        for (i, tile) in map.iter().enumerate() {
            let (x, y) = coords_of(i);

            match tile {
                Tile::Floor => {
                    ctx.set(x, y, RGB::from_f32(0.5, 0.5, 0.5), RGB::from_f32(0., 0., 0.), rltk::to_cp437('.'));
                }
                Tile::Wall => {
                    ctx.set(x, y, RGB::from_f32(0., 1., 0.), RGB::from_f32(0., 0., 0.), rltk::to_cp437('#'));
                }
            }
        }
    }

    // opposite of index_of
    /// Returns the coordinates based on an index.
    fn coords_of(i: usize) -> (i32, i32) {
        ((i % 80) as i32, (i / 80) as i32)
    }
    ```

We'll run it right before rendering the player.
```rust, noplaypen
self.ecs.run_with_data(draw_map, ctx);
```

Final step, we'll transform these ghost walls to concrete by modifying the loop in `move_player`:
```diff
-fn move_player(ctx: &mut Rltk, mut positions: ViewMut<Position>) {
+fn move_player(ctx: &mut Rltk, map: UniqueView<[Tile; 4000]>, mut positions: ViewMut<Position>) {
    let [delta_x, delta_y] = match ctx.key {
        Some(VirtualKeyCode::Left) => [-1, 0],
        Some(VirtualKeyCode::Right) => [1, 0],
        Some(VirtualKeyCode::Up) => [0, -1],
        Some(VirtualKeyCode::Down) => [0, 1],
        _ => return,
    };

    for pos in (&mut positions).iter() {
-        pos.x = min(79, max(0, pos.x + delta_x));
-        pos.y = min(49, max(0, pos.y + delta_y));
+        let destination_idx = index_of(pos.x + delta_x, pos.y + delta_y);
+
+        if let Tile::Floor = map[destination_idx] {
+            pos.x = min(79, max(0, pos.x + delta_x));
+            pos.y = min(49, max(0, pos.y + delta_y));
+        }
    }
}
```

## Final code

```rust, noplaypen
use rltk::{GameState, Rltk, RltkBuilder, VirtualKeyCode, RGB};
use shipyard::{EntitiesViewMut, IntoIter, UniqueView, View, ViewMut, World};
use std::cmp::{max, min};

struct Position {
    x: i32,
    y: i32,
}

struct Renderable {
    glyph: rltk::FontCharType,
    fg: RGB,
    bg: RGB,
}

#[derive(Clone, Copy)]
enum Tile {
    Floor,
    Wall,
}

fn move_player(ctx: &mut Rltk, map: UniqueView<[Tile; 4000]>, mut positions: ViewMut<Position>) {
    let [delta_x, delta_y] = match ctx.key {
        Some(VirtualKeyCode::Left) => [-1, 0],
        Some(VirtualKeyCode::Right) => [1, 0],
        Some(VirtualKeyCode::Up) => [0, -1],
        Some(VirtualKeyCode::Down) => [0, 1],
        _ => return,
    };

    for pos in (&mut positions).iter() {
        let destination_idx = index_of(pos.x + delta_x, pos.y + delta_y);

        if let Tile::Floor = map[destination_idx] {
            pos.x = min(79, max(0, pos.x + delta_x));
            pos.y = min(49, max(0, pos.y + delta_y));
        }
    }
}

fn draw_map(ctx: &mut Rltk, map: UniqueView<[Tile; 4000]>) {
    for (i, tile) in map.iter().enumerate() {
        let (x, y) = coords_of(i);

        match tile {
            Tile::Floor => {
                ctx.set(x, y, RGB::from_f32(0.5, 0.5, 0.5), RGB::from_f32(0., 0., 0.), rltk::to_cp437('.'));
            }
            Tile::Wall => {
                ctx.set(x, y, RGB::from_f32(0., 1., 0.), RGB::from_f32(0., 0., 0.), rltk::to_cp437('#'));
            }
        }
    }
}

struct State {
    ecs: World,
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut Rltk) {
        ctx.cls();

        self.ecs.run_with_data(move_player, ctx);

        self.ecs.run_with_data(draw_map, ctx);
        self.ecs
            .run(|positions: View<Position>, renderables: View<Renderable>| {
                for (pos, render) in (&positions, &renderables).iter() {
                    ctx.set(pos.x, pos.y, render.fg, render.bg, render.glyph);
                }
            });
    }
}

fn initial_entities(
    mut entities: EntitiesViewMut,
    mut positions: ViewMut<Position>,
    mut renderables: ViewMut<Renderable>,
) {
    entities.add_entity(
        (&mut positions, &mut renderables),
        (
            Position { x: 40, y: 25 },
            Renderable {
                glyph: rltk::to_cp437('@'),
                fg: RGB::named(rltk::YELLOW),
                bg: RGB::named(rltk::BLACK),
            },
        ),
    );
}

fn create_map() -> [Tile; 4000] {
    // we start without any wall
    let mut map = [Tile::Floor; 80 * 50];

    // we replace the outer edges with walls
    for x in 0..80 {
        map[index_of(x, 0)] = Tile::Wall;
        map[index_of(x, 49)] = Tile::Wall;
    }
    for y in 1..49 {
        map[index_of(0, y)] = Tile::Wall;
        map[index_of(79, y)] = Tile::Wall;
    }

    let mut rng = rltk::RandomNumberGenerator::new();

    // we randomly place up to 400 walls
    for _ in 0..400 {
        let x = rng.range(0, 80);
        let y = rng.range(0, 50);
        let idx = index_of(x, y);
        if idx != index_of(40, 25) {
            map[idx] = Tile::Wall;
        }
    }

    map
}

/// Returns the index based on coordinates.
fn index_of(x: i32, y: i32) -> usize {
    ((y * 80) + x) as usize
}

/// Returns the coordinates based on an index.
fn coords_of(i: usize) -> (i32, i32) {
    ((i % 80) as i32, (i / 80) as i32)
}

fn main() -> rltk::BError {
    let context = RltkBuilder::simple80x50()
        .with_title("Roguelike Tutorial")
        .build()?;
    let gs = State { ecs: World::new() };

    gs.ecs.run(initial_entities);
    gs.ecs.add_unique(create_map());

    rltk::main_loop(context, gs)
}
```

---

The "Shipyard Tutorial" is a derivative work of ["Roguelike Tutorial - In Rust"](https://bfnightly.bracketproductions.com/rustbook/) by [Herbert Wolverson](https://github.com/thebracket), used under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode). "Shipyard Tutorial" is licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode) by Dylan Ancel.
