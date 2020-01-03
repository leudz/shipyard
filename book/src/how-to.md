# How To

Now that we're ready to use Shipyard, let's learn the basics!

In order to avoid needless repetition, the following is assumed to be predefined in the whole section:

```rust, noplaypen
struct Empty;

#[derive(Debug)]
struct Count(pub u32);

#[derive(Debug)]
struct Position {
    x: f64,
    y: f64
}

#[derive(Debug)]
struct Velocity {
    x: f64,
    y: f64
}

#[derive(Debug)]
pub struct Fruit {
    name: &'static str,
    color: Color, 
}

impl Fruit {
    pub fn new_apple(color: Option<Color>) -> Self {
        Fruit {
            name: "apple",
            color: color.unwrap_or(Color::Red)
        }
    }
    pub fn new_orange() -> Self {
        Fruit {
            name: "orange",
            color: Color::Orange
        }
    }
}

#[derive(Debug)]
pub enum Color {
    Red,
    Orange,
    Green
}
```
