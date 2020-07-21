use crate::components::Position;
use crate::map::{index_of, Map};
use rltk::{field_of_view, Point};
use shipyard::{IntoIter, UniqueViewMut, View, ViewMut};

pub struct Viewshed {
    pub visible_tiles: Vec<Point>,
    pub range: u32,
    pub is_dirty: bool,
}

impl Viewshed {
    pub fn new(range: u32) -> Self {
        Viewshed {
            visible_tiles: Vec::new(),
            range,
            is_dirty: true,
        }
    }
}

pub fn visibility(
    mut map: UniqueViewMut<Map>,
    positions: View<Position>,
    mut viewsheds: ViewMut<Viewshed>,
) {
    for (viewshed, pos) in (&mut viewsheds, &positions).iter() {
        if viewshed.is_dirty {
            viewshed.is_dirty = false;

            viewshed.visible_tiles.clear();
            // query rltk to get all tiles in the player's field of view
            viewshed.visible_tiles =
                field_of_view(Point::new(pos.x, pos.y), viewshed.range as i32, &*map);
            // clip the visible tiles to the console
            viewshed.visible_tiles.retain(|p| {
                p.x >= 0 && p.x < map.width as i32 - 1 && p.y >= 0 && p.y < map.height as i32 - 1
            });

            for t in &mut *map.visible_tiles {
                *t = false;
            }

            for p in &viewshed.visible_tiles {
                map.revealed_tiles[index_of(p.x, p.y)] = true;
                map.visible_tiles[index_of(p.x, p.y)] = true;
            }
        }
    }
}
