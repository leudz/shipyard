use crate::map::{index_of, Tile};
use crate::Position;
use rltk::{Rltk, VirtualKeyCode};
use shipyard::{IntoIter, UniqueView, ViewMut};
use std::cmp::{max, min};

pub fn move_player(
    ctx: &mut Rltk,
    map: UniqueView<[Tile; 4000]>,
    mut positions: ViewMut<Position>,
) {
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
