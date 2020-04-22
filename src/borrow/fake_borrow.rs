use core::marker::PhantomData;

pub struct FakeBorrow<T: ?Sized>(PhantomData<T>);

impl<T: ?Sized> FakeBorrow<T> {
    pub(crate) fn new() -> Self {
        FakeBorrow(PhantomData)
    }
}
