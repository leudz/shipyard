use crate::components::Position;
use crate::map::{index_of, Map, Tile};
use rltk::{Rltk, VirtualKeyCode};
use shipyard::{IntoIter, UniqueView, ViewMut};
use std::cmp::{max, min};

pub fn move_player(ctx: &mut Rltk, map: UniqueView<Map>, mut positions: ViewMut<Position>) {
    let [delta_x, delta_y] = if let Some(key) = ctx.key {
        match key {
            VirtualKeyCode::Left | VirtualKeyCode::H | VirtualKeyCode::Numpad4 => [-1, 0],
            VirtualKeyCode::Right | VirtualKeyCode::L | VirtualKeyCode::Numpad6 => [1, 0],
            VirtualKeyCode::Up | VirtualKeyCode::J | VirtualKeyCode::Numpad8 => [0, -1],
            VirtualKeyCode::Down | VirtualKeyCode::K | VirtualKeyCode::Numpad2 => [0, 1],
            _ => return,
        }
    } else {
        return;
    };

    for pos in (&mut positions).iter() {
        let destination_idx = index_of(pos.x + delta_x, pos.y + delta_y);

        if let Tile::Floor = map.tiles[destination_idx] {
            pos.x = min(map.width as i32 - 1, max(0, pos.x + delta_x));
            pos.y = min(map.height as i32 - 1, max(0, pos.y + delta_y));
        }
    }
}
