use crate::not::Not;

#[derive(Clone)]
pub struct Inserted<Storage>(pub Storage);

impl<Storage> core::ops::Not for Inserted<Storage> {
    type Output = Not<Inserted<Storage>>;

    fn not(self) -> Self::Output {
        Not(self)
    }
}

#[derive(Clone)]
pub struct Modified<Storage>(pub Storage);

impl<Storage> core::ops::Not for Modified<Storage> {
    type Output = Not<Modified<Storage>>;

    fn not(self) -> Self::Output {
        Not(self)
    }
}
#[derive(Clone)]
pub struct InsertedOrModified<Storage>(pub Storage);

impl<Storage> core::ops::Not for InsertedOrModified<Storage> {
    type Output = Not<InsertedOrModified<Storage>>;

    fn not(self) -> Self::Output {
        Not(self)
    }
}
