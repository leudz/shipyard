# A whole new world

We have a map and it's better than the void of space but it's still not super interesting.  
In this chapter we'll enhance the map and in the next one we'll make it more fun to explore.

## Refactoring

Before we start, `main.rs` is getting a little too crowded and it'll only get worse so let's split it in multiple files.

We'll start by moving `Position` and `Renderable` to `src/components.rs`.  
`Tile`, `draw_map`, `create_map`, `index_of` and `coords_of` to `src/map.rs`.  
And `move_player` to `src/player.rs`.

We declare the new modules in `main.rs`:
```rust, noplaypen
mod components;
mod map;
mod player;
```

Then it's just a matter of fixing all imports errors and making everything public. You can do it by yourself or look at the final result [here](https://github.com/leudz/shipyard/tree/master/tutorial/src/bin/3_refactor).

When all errors are fixed you should be able to run the game and have the same output as before.

We'll take the opportunity to make a new type for the map, it'll help us in this chapter.

```rust, noplaypen
pub struct Map {
    pub tiles: [Tile; 4000],
}
```

Now we can move `create_map` and `draw_map` into an `impl Map` block.

```rust, noplaypen
impl Map {
    pub fn draw(ctx: &mut Rltk, map: UniqueView<Self>) {
        for (i, tile) in map.tiles.iter().enumerate() {
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

    pub fn create() -> Self {
        // we start without any wall
        let mut tiles = [Tile::Floor; 80 * 50];

        // we replace the outer edges with walls
        for x in 0..79 {
            tiles[index_of(x, 0)] = Tile::Wall;
            tiles[index_of(x, 49)] = Tile::Wall;
        }
        for y in 1..49 {
            tiles[index_of(0, y)] = Tile::Wall;
            tiles[index_of(79, y)] = Tile::Wall;
        }

        let mut rng = rltk::RandomNumberGenerator::new();

        // we randomly place up to 400 walls
        for _ in 0..400 {
            let x = rng.range(0, 80);
            let y = rng.range(0, 50);
            let idx = index_of(x, y);
            if idx != index_of(40, 25) {
                tiles[idx] = Tile::Wall;
            }
        }

        Map {
            tiles,
        }
    }
}
```

With theses changes we also have to modify the call sites.

- `main.rs`:
    - imports:
        ```diff
        -use map::{create_map, draw_map};
        ```

        ```rust, noplaypen
        use map::Map;
        ```

    - `GameState::tick`:
        ```diff
        -self.ecs.run_with_data(draw_map, ctx);
        ```
        ```rust, noplaypen
        self.ecs.run_with_data(Map::draw, ctx);
        ```

    - `main`:
        ```diff
        -gs.ecs.add_unique(create_map());
        ```
        ```rust, noplaypen
        gs.ecs.add_unique(Map::create());
        ```
- `player.rs`:
    - imports:
        ```diff
        -use crate::map::{index_of, Tile};
        ```
        ```rust, noplaypen
        use crate::map::{index_of, Map, Tile};
        ```
    - `move_player`:
        ```diff
        -pub fn move_player(
        -    ctx: &mut Rltk,
        -    map: UniqueView<[Tile; 4000]>,
        -    mut positions: ViewMut<Position>,
        -) {
        +pub fn move_player(ctx: &mut Rltk, map: UniqueView<Map>, mut positions: ViewMut<Position>) {
            let [delta_x, delta_y] = match ctx.key {
                Some(VirtualKeyCode::Left) => [-1, 0],
                Some(VirtualKeyCode::Right) => [1, 0],
                Some(VirtualKeyCode::Up) => [0, -1],
                Some(VirtualKeyCode::Down) => [0, 1],
                _ => return,
            };

            for pos in (&mut positions).iter() {
                let destination_idx = index_of(pos.x + delta_x, pos.y + delta_y);

        -        if let Tile::Floor = map[destination_idx] {
        +        if let Tile::Floor = map.tiles[destination_idx] {
                    pos.x = min(79, max(0, pos.x + delta_x));
                    pos.y = min(49, max(0, pos.y + delta_y));
                }
            }
        }
        ```

All done!

## To the dungeon!

Just like Rogue, our map will be made of rooms and corridors.  
The simplest way to implement it is to fill the map with walls and dig.

We'll start by adding a struct for the rooms in `map.rs`:
```rust, noplaypen
pub struct Room {
    x1: i32,
    x2: i32,
    y1: i32,
    y2: i32,
}

impl Room {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Room {
        Room {
            x1: x,
            y1: y,
            x2: x + w - 1,
            y2: y + h - 1,
        }
    }

    pub fn intersect(&self, other: &Room) -> bool {
        self.x1 - 1 <= other.x2
            && self.x2 + 1 >= other.x1
            && self.y1 - 1 <= other.y2
            && self.y2 + 1 >= other.y1
    }

    pub fn center(&self) -> [i32; 2] {
        [(self.x1 + self.x2) / 2, (self.y1 + self.y2) / 2]
    }
}
```

In `intersect` we're artificially making one of the room bigger to make sure they always have a wall between them.

We're going to modify the type of our tiles too to be able to make a map of any size:
```diff
-pub struct Map {
-    pub tiles: [Tile; 4000],
-}
```
```rust, noplaypen
pub struct Map {
    pub tiles: Vec<Tile>,
    pub rooms: Vec<Room>,
    pub width: u32,
    pub height: u32,
}
```

A `Box<[Tile]>` would have transcribed our intensions better but `Vec` will make our life easier in a future chapter.

We'll delete `create_map` and a new method to `Map` to generate the dungeon:
```rust, noplaypen
pub fn create_dungeon(width: u32, height: u32) -> Self {
    const MAX_ROOMS: i32 = 30;
    const MIN_SIZE: i32 = 6;
    const MAX_SIZE: i32 = 10;

    // we start with a map full of walls
    let mut tiles = vec![Tile::Wall; (width * height) as usize];

    let mut rng = rltk::RandomNumberGenerator::new();

    let mut rooms: Vec<Room> = Vec::new();
    for _ in 0..MAX_ROOMS {
        let w = rng.range(MIN_SIZE, MAX_SIZE);
        let h = rng.range(MIN_SIZE, MAX_SIZE);
        // we're not allowing rooms to be built on the outer edge
        let x = rng.range(1, width as i32 - 1 - w);
        let y = rng.range(1, height as i32 - 1 - h);
        let new_room = Room::new(x, y, w, h);

        // we go through all previous rooms to check if we overlap one of them
        let mut ok = true;
        for other_room in rooms.iter() {
            if new_room.intersect(other_room) {
                ok = false
            }
        }

        // if the room has a valid location, we add it to the list
        if ok {
            dig_room(&mut tiles, &new_room);
            rooms.push(new_room);
        }
    }

    Map {
        tiles,
        rooms,
        width,
        height,
    }
}
```

New free function:
```rust, noplaypen
fn dig_room(tiles: &mut [Tile], room: &Room) {
    for y in room.y1..=room.y2 {
        for x in room.x1..=room.x2 {
            tiles[index_of(x, y)] = Tile::Floor;
        }
    }
}
```

Finally we modify the call sites:

- `main`:
    ```diff
    -gs.ecs.add_unique(create_map());
    ```
    ```rust, noplaypen
    gs.ecs.add_unique(Map::create_dungeon(80, 50));
    ```

We can run the game again!

Wait a minute... I'm stuck in a wall! And we never made corridors.

We'll start with the corridors by adding two free functions in `map.rs`:
```rust, noplaypen
fn dig_horizontal_tunnel(tiles: &mut [Tile], x1: i32, x2: i32, y: i32) {
    for x in min(x1, x2)..=max(x1, x2) {
        let idx = index_of(x, y);
        if idx > 0 && idx < 80 * 50 {
            tiles[idx as usize] = Tile::Floor;
        }
    }
}

fn dig_vertical_tunnel(tiles: &mut [Tile], y1: i32, y2: i32, x: i32) {
    for y in min(y1, y2)..=max(y1, y2) {
        let idx = index_of(x, y);
        if idx > 0 && idx < 80 * 50 {
            tiles[idx as usize] = Tile::Floor;
        }
    }
}
```

And the imports to go with them:
```rust, noplaypen
use std::cmp::{max, min};
```

We'll use these functions in `create_dungeon`:
```diff
-if ok {
-    dig_room(&mut tiles, &new_room);
-    rooms.push(new_room);
-}
```
```rust, noplaypen
if ok {
    dig_room(&mut tiles, &new_room);

    if let Some(last_room) = rooms.last() {
        let [new_x, new_y] = new_room.center();
        let [prev_x, prev_y] = last_room.center();
        
        if rng.range(0, 2) == 1 {
            dig_horizontal_tunnel(&mut tiles, prev_x, new_x, prev_y);
            dig_vertical_tunnel(&mut tiles, prev_y, new_y, new_x);
        } else {
            dig_vertical_tunnel(&mut tiles, prev_y, new_y, prev_x);
            dig_horizontal_tunnel(&mut tiles, prev_x, new_x, new_y);
        }
    }

    rooms.push(new_room);
}
```

This will connect rooms and alternate their direction.

One problem down, one to go.

To make sure the player never spawns in a wall, we'll put it at the center of a room.

We need to add a new argument to `initial_entities`. It has to be the first argument to stay a system.

- new `initial_entities`'s argument:
    ```diff
    fn initial_entities(
    +    [x, y]: [i32; 2],
        mut entities: EntitiesViewMut,
        mut positions: ViewMut<Position>,
        mut renderables: ViewMut<Renderable>,
    ) {
    ```

- `initial_entities`:
    ```diff
    -Position { x: 40, y: 25 },
    ```
    ```rust, noplaypen
    Position { x, y },
    ```

- `main`:
    ```diff
    -gs.ecs.run(initial_entities);
    -gs.ecs.add_unique(Map::create_dungeon(80, 50));
    ```
    ```rust, noplaypen
    let map = Map::create_dungeon(80, 50);
    gs.ecs
        .run_with_data(initial_entities, map.rooms[0].center());
    gs.ecs.add_unique(map);
    ```

That's it, congratulations! In the next chapter we'll work on exploration.

## Final code

You can find the [final code here](https://github.com/leudz/shipyard/tree/master/tutorial/src/bin/3).

---

The "Shipyard Tutorial" is a derivative work of ["Roguelike Tutorial - In Rust"](https://bfnightly.bracketproductions.com/rustbook/) by [Herbert Wolverson](https://github.com/thebracket), used under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode). "Shipyard Tutorial" is licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode) by Dylan Ancel.
