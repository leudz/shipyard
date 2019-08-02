use crate::sparse_array::{Read, View, ViewMut, Write};
use std::ops::Not as NotOps;

// Used to filter out components
// Get and iterators will skip entities that have this component
#[derive(Copy, Clone)]
pub struct Not<T>(pub(crate) T);

impl<T> Not<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> NotOps for Read<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &Read<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for Write<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &Write<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &mut Write<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for View<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &View<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for ViewMut<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &ViewMut<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}

impl<T> NotOps for &mut ViewMut<'_, T> {
    type Output = Not<Self>;
    fn not(self) -> Self::Output {
        Not(self)
    }
}
