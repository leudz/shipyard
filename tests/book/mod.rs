mod add_components;
mod add_entities;
mod borrow;
mod delete_components;
mod delete_entities;
mod get;
mod hierarchy;
mod iterators;
mod packs;
mod remove_components;
mod syntactic_peculiarities;
#[cfg(feature = "proc")]
mod systems;
mod world;
mod world_insides;

use shipyard::prelude::*;

struct Empty;

#[derive(Debug)]
struct Count(pub u32);

#[derive(Debug)]
struct Position {
    x: f64,
    y: f64,
}

#[derive(Debug)]
struct Velocity {
    x: f64,
    y: f64,
}

#[derive(Debug)]
pub struct Fruit {
    name: &'static str,
    color: Color,
}

impl Fruit {
    pub fn _new_apple(color: Option<Color>) -> Self {
        Fruit {
            name: "apple",
            color: color.unwrap_or(Color::_Red),
        }
    }
    pub fn new_orange() -> Self {
        Fruit {
            name: "orange",
            color: Color::Orange,
        }
    }
}

#[derive(Debug)]
pub enum Color {
    _Red,
    Orange,
    _Green,
}
