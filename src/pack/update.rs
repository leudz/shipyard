#[derive(Clone)]
pub struct Inserted<Storage>(pub Storage);
#[derive(Clone)]
pub struct Modified<Storage>(pub Storage);
#[derive(Clone)]
pub struct InsertedOrModified<Storage>(pub Storage);
