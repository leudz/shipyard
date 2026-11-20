use shipyard::borrow::BorrowInfo as _;
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

struct DefaultOnly(u32);

impl Default for DefaultOnly {
    fn default() -> Self {
        Self(42)
    }
}

#[derive(Component)]
struct Score(u32);

/// Asserts that `Borrow` initializes the marked tuple field and borrows the adjacent view.
#[test]
fn tuple_borrow_default() {
    #[derive(Borrow)]
    struct Scores<'a>(#[shipyard(default)] DefaultOnly, View<'a, Score>);

    let mut world = World::new();
    let entity = world.add_entity(Score(7));
    let all_storages = world.all_storages().unwrap();
    let Scores(DefaultOnly(default), scores) = all_storages.borrow::<Scores>().unwrap();

    assert_eq!(default, 42);
    assert_eq!(scores[entity].0, 7);
}

/// Asserts that `WorldBorrow` initializes the marked tuple field and `BorrowInfo` schedules
/// and enables insertion tracking for the real view.
#[test]
fn tuple_world_borrow_default() {
    #[derive(WorldBorrow, BorrowInfo)]
    struct Scores<'a>(
        View<'a, Score, track::Insertion>,
        #[shipyard(default)] DefaultOnly,
    );

    fn read_scores(Scores(scores, DefaultOnly(default)): Scores<'_>) {
        assert_eq!(default, 42);
        assert_eq!(
            scores
                .inserted()
                .iter()
                .map(|score| score.0)
                .collect::<Vec<_>>(),
            [7]
        );
    }

    let mut world = World::new();
    Workload::new("scores")
        .with_system(read_scores)
        .add_to_world(&world)
        .unwrap();
    world.add_entity(Score(7));
    world.run_workload("scores").unwrap();

    let mut expected = Vec::new();
    View::<Score, track::Insertion>::borrow_info(&mut expected);
    let info = world.workloads_info();
    let system = info.0["scores"].batches_info[0].systems().next().unwrap();
    assert_eq!(system.borrow, expected);
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

/// Checks that field visibility applies to the generated iterator.
#[test]
fn into_iter_field_visibility() {
    mod inner {
        use super::*;

        #[derive(Component)]
        pub struct A;

        #[derive(Component)]
        pub struct B;

        #[derive(IntoIter, Borrow)]
        pub struct CustomView<'a> {
            pub(super) comp_a: shipyard::View<'a, A>,
            pub(crate) v_comp_aa: shipyard::View<'a, A>,
            pub comp_b: shipyard::ViewMut<'a, B>,
            vm_comp_bb: shipyard::ViewMut<'a, B>,
        }
    }

    let world = World::new();
    let Ok(view) = world.borrow::<inner::CustomView>() else {
        // This function always returns here since 2 ViewMut cannot be borrowed
        // This test is only a compile check, not runtime

        return;
    };

    view.comp_a;
    view.comp_b;
    view.v_comp_aa;
    // Should not be accessible
    // view.v_comp_bb;
}
