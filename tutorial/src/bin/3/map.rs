use rltk::{Rltk, RGB};
use shipyard::UniqueView;
use std::cmp::{max, min};

#[derive(Clone, Copy)]
pub enum Tile {
    Floor,
    Wall,
}

#[allow(unused)]
pub struct Map {
    pub tiles: Vec<Tile>,
    pub rooms: Vec<Room>,
    pub width: u32,
    pub height: u32,
}

impl Map {
    #[rustfmt::skip]
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

    pub fn create_dungeon(width: u32, height: u32) -> Self {
        const MAX_ROOMS: i32 = 30;
        const MIN_SIZE: i32 = 6;
        const MAX_SIZE: i32 = 10;

        // we start with a map full of walls
        let mut tiles = vec![Tile::Wall; (width * height) as usize];

        let mut rooms: Vec<Room> = Vec::new();

        let mut rng = rltk::RandomNumberGenerator::new();

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
        }

        Map {
            tiles,
            rooms,
            width,
            height,
        }
    }
}

fn dig_room(tiles: &mut [Tile], room: &Room) {
    for y in room.y1..=room.y2 {
        for x in room.x1..=room.x2 {
            tiles[index_of(x, y)] = Tile::Floor;
        }
    }
}

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

pub fn index_of(x: i32, y: i32) -> usize {
    ((y * 80) + x) as usize
}

pub fn coords_of(i: usize) -> (i32, i32) {
    ((i % 80) as i32, (i / 80) as i32)
}

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
