mod packed;

pub trait AbstractMut<'a>: Copy {
    type Out;
    // # Safety
    // The reference has to point to something valid
    // The lifetime has to be valid
    unsafe fn add(self, count: usize) -> Self::Out;
}

impl<'a, T: 'a> AbstractMut<'a> for *const T {
    type Out = &'a T;
    unsafe fn add(self, count: usize) -> Self::Out {
        &*self.add(count)
    }
}

impl<'a, T: 'a> AbstractMut<'a> for *mut T {
    type Out = &'a mut T;
    unsafe fn add(self, count: usize) -> Self::Out {
        &mut *self.add(count)
    }
}
