# Using this Guide

In order to avoid needless repitition with demo code, the following is assumed to be predefined in all examples:

### Components
```
pub struct Empty {}

#[derive(Debug)]
pub struct Count (pub u32);

#[derive(Debug)]
pub struct Position {x: f64, y: f64}

#[derive(Debug)]
pub struct Velocity {x: f64, y: f64}

#[derive(Debug)]
pub struct Fruit {
    name: &'static str,
    color: Color, 
}

impl Fruit {
    pub fn new_apple(color: Option<Color>) -> Self {
        Self { name: "apple", color: color.unwrap_or(Color::Red) }
    }
    pub fn new_orange() -> Self {
        Self { name: "orange", color: Color::Orange}
    }
}

#[derive(Debug)]
pub enum Color {
    Red,
    Orange,
    Green
}
```