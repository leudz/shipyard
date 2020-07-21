use rltk::RGB;

pub struct Position {
    pub x: i32,
    pub y: i32,
}

pub struct Renderable {
    pub glyph: rltk::FontCharType,
    pub fg: RGB,
    pub bg: RGB,
}
