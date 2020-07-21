use rltk::{Rltk, RGB};
use shipyard::UniqueView;

#[derive(Clone, Copy)]
pub enum Tile {
    Floor,
    Wall,
}

#[rustfmt::skip]
pub fn draw_map(ctx: &mut Rltk, map: UniqueView<[Tile; 4000]>) {
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

pub fn create_map() -> [Tile; 4000] {
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
pub fn index_of(x: i32, y: i32) -> usize {
    ((y * 80) + x) as usize
}

/// Returns the coordinates based on an index.
pub fn coords_of(i: usize) -> (i32, i32) {
    ((i % 80) as i32, (i / 80) as i32)
}
