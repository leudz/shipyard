# The journey begins

So far we've always had the whole map as soon as the game begins which makes any exploration pointless.  
We'll change that in this chapter by making the player only see what is in their field of view.

We'll start by implementing a couple of trait for `Map`:
- imports:
    ```diff
    -use rltk::{Rltk, RGB};
    ```
    ```rust, noplaypen
    use rltk::{Algorithm2D, BaseMap, Point, Rltk, RGB};
    ```
- `Tile` derive:
    ```rust, noplaypen
    #[derive(Clone, Copy, PartialEq, Eq)]
    ```
- new `impl`:
    ```rust, noplaypen
    impl BaseMap for Map {
        fn is_opaque(&self, idx: usize) -> bool {
            self.tiles[idx] == Tile::Wall
        }
    }

    impl Algorithm2D for Map {
        fn dimensions(&self) -> Point {
            Point::new(self.width, self.height)
        }
    }
    ```
    With these two traits we'll be able to query rltk to know what the player sees.

We'll make a new `visibility.rs` file and add:
- imports:
    ```rust, noplaypen
    use crate::components::Position;
    use crate::map::Map;
    use rltk::{field_of_view, Point};
    use shipyard::{IntoIter, UniqueView, View, ViewMut};
    ```
- new component:
    ```rust, noplaypen
    pub struct Viewshed {
        pub visible_tiles: Vec<Point>,
        pub range: u32,
    }

    impl Viewshed {
        pub fn new(range: u32) -> Self {
            Viewshed {
                visible_tiles: Vec::new(),
                range,
            }
        }
    }
    ```
- new system:
    ```rust, noplaypen
    pub fn visibility(
        map: UniqueView<Map>,
        positions: View<Position>,
        mut viewsheds: ViewMut<Viewshed>,
    ) {
        for (viewshed, pos) in (&mut viewsheds, &positions).iter() {
            viewshed.visible_tiles.clear();
            // query rltk to get all tiles in the player's field of view
            viewshed.visible_tiles =
                field_of_view(Point::new(pos.x, pos.y), viewshed.range as i32, &*map);
            // clip the visible tiles to the console
            viewshed.visible_tiles.retain(|p| {
                p.x >= 0 && p.x < map.width as i32 - 1 && p.y >= 0 && p.y < map.height as i32 - 1
            });
        }
    }
    ```

To make it work we need to add the new component to the player in `main.rs` and run the system:
- module:
    ```rust, noplaypen
    mod visibility;
    ```
- imports:
    ```rust, noplaypen
    use visibility::{visibility, Viewshed};
    ```
- `ìnitial_entities`:
    ```diff
    fn initial_entities(
        [x, y]: [i32; 2],
        mut entities: EntitiesViewMut,
        mut positions: ViewMut<Position>,
        mut renderables: ViewMut<Renderable>,
    +    mut viewsheds: ViewMut<Viewshed>,
    ) {
        entities.add_entity(
    -        (&mut positions, &mut renderables),
    +        (&mut positions, &mut renderables, &mut viewsheds),
            (
                Position { x, y },
                Renderable {
                    glyph: rltk::to_cp437('@'),
                    fg: RGB::named(rltk::YELLOW),
                    bg: RGB::named(rltk::BLACK),
                },
    +            Viewshed::new(8),
            ),
        );
    }
    ```
- `State::tick`:
    ```rust, noplaypen
    self.ecs.run(visibility);
    ```
    We'll run this system between moving the player and rendering the map.

Last step is to modify `map.rs`:
- imports:
    ```diff
    -use shipyard::UniqueView;
    ```
    ```rust, noplaypen
    use crate::visibility::Viewshed;
    use shipyard::{IntoIter, UniqueView, View};
    ```
- `Map::draw`:
    ```rust, noplaypen
    pub fn draw(ctx: &mut Rltk, map: UniqueView<Self>, viewsheds: View<Viewshed>) {
        for (i, tile) in map.tiles.iter().enumerate() {
            let (x, y) = coords_of(i);
            let p = Point::new(x, y);

            for viewshed in viewsheds.iter() {
                if viewshed.visible_tiles.contains(&p) {
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
        }
    }
    ```
    We're only rendering visible tiles now.

## I'm not a goldfish!

We have what we wanted, we can only see the tiles our character can. But it's a bit too extreme, the walls are not going to move.  
So let's make the game help us remember where there are.

- `map.rs`:
    - `Map` new field:
        ```rust, noplaypen
        pub revealed_tiles: Vec<bool>,
        ```
    - `Map::create_dungeon`:
        ```diff
        Map {
            tiles,
            rooms,
        +    revealed_tiles: vec![false; (width * height) as usize],
            width,
            height,
        }
        ```
    - `Map::draw`:
        ```diff
        pub fn draw(ctx: &mut Rltk, map: UniqueView<Self>, viewsheds: View<Viewshed>) {
            for (i, tile) in map.tiles.iter().enumerate() {
                let (x, y) = coords_of(i);
        -        let p = Point::new(x, y);
        
        -        for viewshed in viewsheds.iter() {
        -            if viewshed.visible_tiles.contains(&p) {
        +        if map.revealed_tiles[i] {
                    match tile {
                        Tile::Floor => {
                            ctx.set(x, y, RGB::from_f32(0.5, 0.5, 0.5), RGB::from_f32(0., 0., 0.), rltk::to_cp437('.'));
                        }
                        Tile::Wall => {
                            ctx.set(x, y, RGB::from_f32(0., 1., 0.), RGB::from_f32(0., 0., 0.), rltk::to_cp437('#'));
                        }
                    }
                }
        -       }
            }
        }
        ```
        There will be some unused imports that you can remove.
- `visibility.rs`:
    - imports:
        ```diff
        -use crate::map::Map;
        -use shipyard::{IntoIter, UniqueView, View, ViewMut};
        ```
        ```rust, noplaypen
        use crate::map::{index_of, Map};
        use shipyard::{IntoIter, UniqueViewMut, View, ViewMut};
        ```
    - `visibility`:
        ```diff
        -map: UniqueView<Map>,
        ```
        ```rust, noplaypen
        mut map: UniqueViewMut<Map>,
        ```
        ```diff
        viewshed.visible_tiles.clear();
        // query rltk to get all tiles in the player's field of view
        viewshed.visible_tiles =
            field_of_view(Point::new(pos.x, pos.y), viewshed.range, &*map);
        // clip the visible tiles to the console
        viewshed.visible_tiles.retain(|p| {
            p.x >= 0 && p.x < map.width as i32 - 1 && p.y >= 0 && p.y < map.height as i32 - 1
        });

        +for p in &viewshed.visible_tiles {
        +    map.revealed_tiles[index_of(p.x, p.y)] = true;
        +}
        ```
        We're just setting all tiles viewed as revealed.

We're currently recalculating the player's field of view each frame but most of the time they're not moving.  
We'll add a dirty flag to only recalculate it when needed.

- `visibility.rs`:
    - `Viewshed` new field:
        ```rust, noplaypen
        pub is_dirty: bool,
        ```
    - `Viewshed::new` new field:
        ```diff
        Viewshed {
        +    is_dirty: true,
            visible_tiles: Vec::new(),
            range,
        }
        ```
    - `visibility`:
        ```diff
        +if viewshed.is_dirty {
        +    viewshed.is_dirty = false;
        +
            viewshed.visible_tiles.clear();
            // query rltk to get all tiles in the player's field of view
            viewshed.visible_tiles =
                field_of_view(Point::new(pos.x, pos.y), viewshed.range as i32, &*map);
            // clip the visible tiles to the console
            viewshed.visible_tiles.retain(|p| {
                p.x >= 0 && p.x < map.width as i32 - 1 && p.y >= 0 && p.y < map.height as i32 - 1
            });

            for p in &viewshed.visible_tiles {
                map.revealed_tiles[index_of(p.x, p.y)] = true;
            }
        +}
        ```
        Now we check the flag before doing anything and reset it when it's dirty.
- `player.rs`:
    - imports:
        ```rust, noplaypen
        use crate::visibility::Viewshed;
        ```
    - `move_player`:
        ```diff
        -pub fn move_player(ctx: &mut Rltk, map: UniqueView<Map>, mut positions: ViewMut<Position>) {
        +pub fn move_player(
        +    ctx: &mut Rltk,
        +    map: UniqueView<Map>,
        +    mut positions: ViewMut<Position>,
        +    mut viewsheds: ViewMut<Viewshed>,
        +) {
            let [delta_x, delta_y] = match ctx.key {
                Some(VirtualKeyCode::Left) => [-1, 0],
                Some(VirtualKeyCode::Right) => [1, 0],
                Some(VirtualKeyCode::Up) => [0, -1],
                Some(VirtualKeyCode::Down) => [0, 1],
                _ => return,
            };

        -    for pos in (&mut positions).iter() {
        +    for (pos, viewshed) in (&mut positions, &mut viewsheds).iter() {
                let destination_idx = index_of(pos.x + delta_x, pos.y + delta_y);

                if let Tile::Floor = map.tiles[destination_idx] {
                    pos.x = min(map.width as i32 - 1, max(0, pos.x + delta_x));
                    pos.y = min(map.height as i32 - 1, max(0, pos.y + delta_y));
                }

        +        viewshed.is_dirty = true;
            }
        }
        ```
        We've added `Viewshed` to the system and made the flag dirty when the player moves.

Last step is to differentiate between what we see and what we saw.
- `map.rs`:
    - `Map` new field:
        ```rust, noplaypen
        pub visible_tiles: Vec<bool>,
        ```
    - `create_dungeon`:
        ```diff
        Map {
            tiles,
            rooms,
            revealed_tiles: vec![false; (width * height) as usize],
        +    visible_tiles: vec![false; (width * height) as usize],
            width,
            height,
        }
        ```
    - `Map::draw`:
        ```rust, noplaypen
        pub fn draw(ctx: &mut Rltk, map: UniqueView<Self>) {
            for (i, tile) in map.tiles.iter().enumerate() {
                if map.revealed_tiles[i] {
                    let (x, y) = coords_of(i);

                    let (glyph, mut fg) = match tile {
                        Tile::Floor => (rltk::to_cp437('.'), RGB::from_f32(0., 0.5, 0.5)),
                        Tile::Wall => (rltk::to_cp437('#'), RGB::from_f32(0., 1., 0.)),
                    };

                    if !map.visible_tiles[i] {
                        fg = fg.to_greyscale();
                    }

                    ctx.set(x, y, fg, RGB::from_f32(0., 0., 0.), glyph);
                }
            }
        }
        ```
        We made the visible floor a little more visible and we greyed out what we can't see.
- `visibility.rs`:
    - `visibility`:
        ```diff
        +for t in &mut *map.visible_tiles {
        +    *t = false;
        +}

        for p in &viewshed.visible_tiles {
            map.revealed_tiles[index_of(p.x, p.y)] = true;
        +    map.visible_tiles[index_of(p.x, p.y)] = true;
        }
        ```

Have fun exploring your dungeons! They are safe... for now.

## Final code

You can find the [final code here](https://github.com/leudz/shipyard/tree/master/tutorial/src/bin/4).

---

The "Shipyard Tutorial" is a derivative work of ["Roguelike Tutorial - In Rust"](https://bfnightly.bracketproductions.com/rustbook/) by [Herbert Wolverson](https://github.com/thebracket), used under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode). "Shipyard Tutorial" is licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode) by Dylan Ancel.
