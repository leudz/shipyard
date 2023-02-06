use shipyard::*;

#[cfg(feature = "proc")]
#[test]
fn check_derive() {
    #[derive(Component, Unique, Borrow, AllStoragesBorrow, BorrowInfo)]
    struct A {
        _a: (),
    }

    #[derive(Component, Unique, Borrow, AllStoragesBorrow, BorrowInfo)]
    struct B(A);

    #[derive(Component, Unique)]
    struct C;
}
