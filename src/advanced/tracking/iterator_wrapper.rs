use crate::not::Not;

/// Iterates components inserted in the view's tracking interval.
///
/// ```
/// use shipyard::{Component, IntoIter, View, World, iter::Inserted, track};
///
/// struct Item;
/// impl Component for Item { type Tracking = track::Insertion; }
///
/// let mut world = World::new();
/// world.add_entity(Item);
///
/// let view = world.borrow::<View<Item>>().unwrap();
/// assert_eq!(Inserted(&view).iter().count(), 1);
/// assert_eq!((!Inserted(&view)).iter().count(), 0);
/// ```
///
/// Only views tracking insertion can be iterated this way:
///
/// ```compile_fail
/// use shipyard::{Component, IntoIter, View, World, iter::Inserted, track};
///
/// struct Item;
/// impl Component for Item { type Tracking = track::Untracked; }
///
/// let mut world = World::new();
/// world.add_entity(Item);
///
/// let view = world.borrow::<View<Item>>().unwrap();
/// Inserted(&view).iter().count();
/// ```
#[derive(Clone)]
pub struct Inserted<Storage>(pub Storage);

impl<Storage> core::ops::Not for Inserted<Storage> {
    type Output = Not<Inserted<Storage>>;

    fn not(self) -> Self::Output {
        Not(self)
    }
}

/// Iterates components modified in the view's tracking interval.
///
/// ```
/// use shipyard::{Component, IntoIter, View, World, iter::Modified, track};
///
/// struct Item;
/// impl Component for Item { type Tracking = track::Modification; }
///
/// let mut world = World::new();
/// world.add_entity((Item,));
///
/// let view = world.borrow::<View<Item>>().unwrap();
/// let _ = Modified(&view).iter().count();
/// let _ = (!Modified(&view)).iter().count();
/// ```
#[derive(Clone)]
pub struct Modified<Storage>(pub Storage);

impl<Storage> core::ops::Not for Modified<Storage> {
    type Output = Not<Modified<Storage>>;

    fn not(self) -> Self::Output {
        Not(self)
    }
}

/// Iterates components with an enabled insertion or modification event in the view's interval.
///
/// ```
/// use shipyard::{Component, IntoIter, View, World, iter::InsertedOrModified, track};
///
/// struct Item;
/// impl Component for Item { type Tracking = track::Insertion; }
///
/// let mut world = World::new();
/// world.add_entity((Item,));
///
/// let view = world.borrow::<View<Item>>().unwrap();
/// assert_eq!(InsertedOrModified(&view).iter().count(), 1);
/// assert_eq!((!InsertedOrModified(&view)).iter().count(), 0);
/// ```
#[derive(Clone)]
pub struct InsertedOrModified<Storage>(pub Storage);

impl<Storage> core::ops::Not for InsertedOrModified<Storage> {
    type Output = Not<InsertedOrModified<Storage>>;

    fn not(self) -> Self::Output {
        Not(self)
    }
}
