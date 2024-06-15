use shipyard::*;

#[test]
fn check_derive() {
    #[derive(Component, Unique, Borrow, BorrowInfo)]
    struct A {
        _a: (),
        _b: (),
        _c: (),
        _d: (),
    }

    #[derive(Component, Unique, Borrow, BorrowInfo)]
    struct B(A);

    #[derive(WorldBorrow)]
    struct C {
        _c: (),
    }

    #[derive(WorldBorrow)]
    struct D(C);

    #[derive(Component, Unique)]
    struct E;

    #[derive(IntoIter)]
    struct CustomView<'a> {
        comp_a: shipyard::View<'a, A>,
        v_comp_aa: shipyard::View<'a, A>,
        comp_b: shipyard::ViewMut<'a, B>,
        vm_comp_bb: shipyard::ViewMut<'a, B>,
    }

    #[derive(IntoIter)]
    struct CustomView2<'a>(shipyard::View<'a, A>, shipyard::ViewMut<'a, B>);
}

/// Verify that the "default" attribute compiles.
/// And that we can borrow a view containing a default field.
#[test]
fn default() {
    #[derive(Borrow)]
    struct CustomView {
        #[shipyard(default)]
        _vec: Vec<usize>,
    }

    let world = World::new();

    world.borrow::<CustomView>().unwrap();
}

#[derive(Hash, Debug, PartialEq, Clone, Label)]
struct MyLabel;
