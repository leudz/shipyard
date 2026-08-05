use crate::component::Component;
use crate::views::ViewMut;
use core::any::TypeId;

/// Groups component storages.
pub trait Group {
    /// Groups component storages.\
    /// Grouped components are iterated faster at the cost of slower insertion and deletion.
    ///
    /// ### Example
    /// ```
    /// use shipyard::{Component, Group, ViewMut, World};
    ///
    /// #[derive(Component)]
    /// struct A;
    ///
    /// #[derive(Component)]
    /// struct B;
    ///
    /// let mut world = World::new();
    ///
    /// let mut views = world.borrow::<(ViewMut<A>, ViewMut<B>)>().unwrap();
    /// views.create_group();
    /// ```
    fn create_group(&mut self);
}

macro_rules! impl_group {
    ($(($component:ident, $track:ident, $index:tt))+) => {
        impl<$($component: Component, $track),+> Group for ($(ViewMut<'_, $component, $track>,)+) {
            #[inline]
            fn create_group(&mut self) {
                let type_ids = [$(TypeId::of::<$component>(),)+];

                $({
                    let mut group = type_ids;
                    group[$index..].rotate_left(1);

                    let group_len = group.len();
                    let group = &mut group[..group_len - 1];
                    group.sort_unstable();

                    self.$index.sparse_set.add_group(group);
                })+
            }
        }
    };
}

macro_rules! group {
    ($(($component:ident, $track:ident, $index:tt))+; ($component1:ident, $track1:ident, $index1:tt) $(($queue_component:ident, $queue_track:ident, $queue_index:tt))*) => {
        impl_group![$(($component, $track, $index))*];
        group![$(($component, $track, $index))* ($component1, $track1, $index1); $(($queue_component, $queue_track, $queue_index))*];
    };
    ($(($component:ident, $track:ident, $index:tt))+;) => {
        impl_group![$(($component, $track, $index))*];
    };
}

group![
    (ComponentA, TrackA, 0) (ComponentB, TrackB, 1);
    (ComponentC, TrackC, 2) (ComponentD, TrackD, 3) (ComponentE, TrackE, 4) (ComponentF, TrackF, 5)
    (ComponentG, TrackG, 6) (ComponentH, TrackH, 7) (ComponentI, TrackI, 8) (ComponentJ, TrackJ, 9)
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{track, World};

    struct A;
    struct B;
    struct C;
    struct D;
    struct E;

    impl Component for A {
        type Tracking = track::Untracked;
    }
    impl Component for B {
        type Tracking = track::Untracked;
    }
    impl Component for C {
        type Tracking = track::Untracked;
    }
    impl Component for D {
        type Tracking = track::Untracked;
    }
    impl Component for E {
        type Tracking = track::Untracked;
    }

    #[test]
    fn creates_group_without_own_component_type() {
        let world = World::new();

        let mut views = world
            .borrow::<(
                ViewMut<'_, A>,
                ViewMut<'_, B>,
                ViewMut<'_, C>,
                ViewMut<'_, D>,
                ViewMut<'_, E>,
            )>()
            .unwrap();

        views.create_group();

        let a = TypeId::of::<A>();
        let b = TypeId::of::<B>();
        let c = TypeId::of::<C>();
        let d = TypeId::of::<D>();
        let e = TypeId::of::<E>();

        let mut group_a = [b, c, d, e];
        let mut group_b = [a, c, d, e];
        let mut group_c = [a, b, d, e];
        let mut group_d = [a, b, c, e];
        let mut group_e = [a, b, c, d];

        group_a.sort_unstable();
        group_b.sort_unstable();
        group_c.sort_unstable();
        group_d.sort_unstable();
        group_e.sort_unstable();

        assert_eq!(&views.0.sparse_set.groups[0], &group_a);
        assert_eq!(&views.1.sparse_set.groups[0], &group_b);
        assert_eq!(&views.2.sparse_set.groups[0], &group_c);
        assert_eq!(&views.3.sparse_set.groups[0], &group_d);
        assert_eq!(&views.4.sparse_set.groups[0], &group_e);
    }
}
