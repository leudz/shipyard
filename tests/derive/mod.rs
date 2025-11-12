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

    #[allow(unused, reason = "This only checks the proc-macro compiles")]
    #[derive(WorldBorrow)]
    struct C {
        _c: (),
    }

    #[allow(unused, reason = "This only checks the proc-macro compiles")]
    #[derive(WorldBorrow)]
    struct D(C);

    #[allow(unused, reason = "This only checks the proc-macro compiles")]
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

#[allow(unused)]
#[derive(Hash, Debug, PartialEq, Clone, Label)]
struct MyLabel;

#[test]
fn into_iter_rename() {
    #[derive(Component)]
    struct A;
    #[derive(Component)]
    struct B;
    #[derive(Component)]
    struct C;

    #[derive(Borrow, IntoIter)]
    #[shipyard(item_name = "OtherName")]
    struct AView<'a> {
        #[shipyard(item_field_name = "some_name")]
        a: View<'a, A>,
        #[shipyard(item_field_name = "some_other_name")]
        vm_b: ViewMut<'a, B>,

        #[shipyard(item_field_skip)]
        _aa: View<'a, A>,
        #[shipyard(item_field_skip)]
        _vm_c: ViewMut<'a, C>,
    }

    let world = World::new();

    world.run(|mut custom: AView| {
        for a in custom.iter() {
            let OtherName {
                some_name: _,
                some_other_name: _,
            } = a;
        }
    });
}
