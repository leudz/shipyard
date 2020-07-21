use crate::components::Position;
use crate::map::{index_of, Map, Tile};
use crate::visibility::Viewshed;
use rltk::{Rltk, VirtualKeyCode};
use shipyard::{IntoIter, UniqueView, ViewMut};
use std::cmp::{max, min};

pub fn move_player(
    ctx: &mut Rltk,
    map: UniqueView<Map>,
    mut positions: ViewMut<Position>,
    mut viewsheds: ViewMut<Viewshed>,
) {
    let [delta_x, delta_y] = match ctx.key {
        Some(VirtualKeyCode::Left) => [-1, 0],
        Some(VirtualKeyCode::Right) => [1, 0],
        Some(VirtualKeyCode::Up) => [0, -1],
        Some(VirtualKeyCode::Down) => [0, 1],
        _ => return,
    };

    for (pos, viewshed) in (&mut positions, &mut viewsheds).iter() {
        let destination_idx = index_of(pos.x + delta_x, pos.y + delta_y);

        if let Tile::Floor = map.tiles[destination_idx] {
            pos.x = min(map.width as i32 - 1, max(0, pos.x + delta_x));
            pos.y = min(map.height as i32 - 1, max(0, pos.y + delta_y));
        }

        viewshed.is_dirty = true;
    }
}
