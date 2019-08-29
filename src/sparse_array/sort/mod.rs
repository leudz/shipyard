use crate::sparse_array::ViewMut;
use std::cmp::Ordering;
use std::ptr;

#[allow(dead_code)]
fn insertion_sort<T, F>(view: &mut ViewMut<T>, cmp: &mut F)
where
    F: FnMut(&T, &T) -> Ordering,
{
    // put data in the right order
    // dense mimics its ordering
    // at the end go through all modified dense indices and update sparse

    let mut lowest_shift = view.dense.len();
    for i in 2..view.dense.len() + 1 {
        shift_tail(view, cmp, i, &mut lowest_shift);
    }

    for i in lowest_shift..view.dense.len() {
        unsafe {
            *view
                .sparse
                .get_unchecked_mut(view.dense.get_unchecked(i).index()) = i;
        }
    }
}

fn shift_tail<T, F>(view: &mut ViewMut<T>, cmp: &mut F, len: usize, lowest_shift: &mut usize)
where
    F: FnMut(&T, &T) -> Ordering,
{
    if len >= 2
        && cmp(unsafe { view.data.get_unchecked(len - 1) }, unsafe {
            view.data.get_unchecked(len - 2)
        }) == Ordering::Less
    {
        let mut index = len - 2;
        while index >= 1
            && cmp(unsafe { view.data.get_unchecked(len - 1) }, unsafe {
                view.data.get_unchecked(index - 1)
            }) == Ordering::Less
        {
            index -= 1;
        }

        unsafe {
            let dense = *view.dense.get_unchecked(len - 1);
            let data = ptr::read(view.data.get_unchecked(len - 1));

            ptr::copy(
                view.dense.get_unchecked(index),
                view.dense.get_unchecked_mut(index + 1),
                len - 1 - index,
            );
            ptr::copy(
                view.data.get_unchecked(index),
                view.data.get_unchecked_mut(index + 1),
                len - 1 - index,
            );
            *view.dense.get_unchecked_mut(index) = dense;
            *view.data.get_unchecked_mut(index) = data;
            *lowest_shift = std::cmp::min(*lowest_shift, index);
        }
    }
}

#[allow(dead_code)]
fn heapsort<T: std::fmt::Debug, F: FnMut(&T, &T) -> Ordering>(view: &mut ViewMut<T>, cmp: &mut F) {
    // find the biggest leaf node
    fn leaf_search<T, F: FnMut(&T, &T) -> Ordering>(
        view: &mut ViewMut<T>,
        mut i: usize,
        len: usize,
        cmp: &mut F,
    ) -> usize {
        while i * 2 + 2 <= len {
            i = if cmp(unsafe { view.data.get_unchecked(i * 2 + 1) }, unsafe {
                view.data.get_unchecked(i * 2 + 2)
            }) == Ordering::Greater
            {
                i * 2 + 1
            } else {
                i * 2 + 2
            };
        }

        if i * 2 + 1 <= len {
            i = i * 2 + 1;
        }

        i
    };
    fn sift_down<T, F: FnMut(&T, &T) -> Ordering>(
        view: &mut ViewMut<T>,
        i: usize,
        len: usize,
        cmp: &mut F,
    ) {
        let mut leaf = leaf_search(view, i, len, cmp);

        // now that we have the biggest leaf we find where data[i] will fit in its parents
        while cmp(unsafe { view.data.get_unchecked(i) }, unsafe {
            view.data.get_unchecked(leaf)
        }) == Ordering::Greater
        {
            leaf = (leaf - 1) / 2;
        }

        // we put data[i] in its rightful place and "shift" all parent nodes
        // to fill the hole
        let mut dense = unsafe { ptr::read(view.dense.get_unchecked(leaf)) };
        unsafe {
            ptr::copy(
                view.dense.get_unchecked(i),
                view.dense.get_unchecked_mut(leaf),
                1,
            )
        };
        let mut data = unsafe { ptr::read(view.data.get_unchecked(leaf)) };
        unsafe {
            ptr::copy(
                view.data.get_unchecked(i),
                view.data.get_unchecked_mut(leaf),
                1,
            )
        };
        while leaf > i {
            leaf = (leaf - 1) / 2;
            unsafe { ptr::swap(&mut dense, view.dense.get_unchecked_mut(leaf)) };
            unsafe { ptr::swap(&mut data, view.data.get_unchecked_mut(leaf)) };
        }
    };

    for i in (0..view.dense.len() / 2).rev() {
        sift_down(view, i, view.dense.len(), cmp);
    }

    for i in (1..view.dense.len()).rev() {
        view.dense.swap(0, i);
        view.data.swap(0, i);
        sift_down(view, 0, i - 1, cmp);
    }

    for i in 0..view.dense.len() {
        unsafe {
            *view
                .sparse
                .get_unchecked_mut(view.dense.get_unchecked(i).index()) = i;
        }
    }
}

#[test]
fn insert_sort() {
    let mut array = super::SparseArray::default();

    for i in (0..100).rev() {
        let mut key = crate::entity::Key::zero();
        key.set_index(100 - i);
        array.view_mut().insert(i, key);
    }

    insertion_sort(&mut array.view_mut(), &mut |x: &usize, y: &usize| x.cmp(&y));

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..100 {
        let mut key = crate::entity::Key::zero();
        key.set_index(100 - i);
        assert_eq!(array.get(key), Some(&i));
    }
}

#[test]
fn insert_sort_partially_sorted() {
    let mut array = super::SparseArray::default();

    for i in 0..20 {
        let mut key = crate::entity::Key::zero();
        key.set_index(i);
        assert!(array.view_mut().insert(i, key).is_none());
    }
    for i in (20..100).rev() {
        let mut key = crate::entity::Key::zero();
        key.set_index(100 - i + 20);
        assert!(array.view_mut().insert(i, key).is_none());
    }

    insertion_sort(&mut array.view_mut(), &mut |x: &usize, y: &usize| x.cmp(&y));

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..20 {
        let mut key = crate::entity::Key::zero();
        key.set_index(i);
        assert_eq!(array.get(key), Some(&i));
    }
    for i in 20..100 {
        let mut key = crate::entity::Key::zero();
        key.set_index(100 - i + 20);
        assert_eq!(array.get(key), Some(&i));
    }
}

#[test]
fn heap_sort() {
    let mut array = super::SparseArray::default();

    for i in (0..100).rev() {
        let mut key = crate::entity::Key::zero();
        key.set_index(100 - i);
        array.view_mut().insert(i, key);
    }

    heapsort(&mut array.view_mut(), &mut |x: &usize, y: &usize| x.cmp(&y));

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..100 {
        let mut key = crate::entity::Key::zero();
        key.set_index(100 - i);
        assert_eq!(array.get(key), Some(&i));
    }
}

#[test]
fn heap_sort_partially_sorted() {
    let mut array = super::SparseArray::default();

    for i in 0..20 {
        let mut key = crate::entity::Key::zero();
        key.set_index(i);
        assert!(array.view_mut().insert(i, key).is_none());
    }
    for i in (20..100).rev() {
        let mut key = crate::entity::Key::zero();
        key.set_index(100 - i + 20);
        assert!(array.view_mut().insert(i, key).is_none());
    }

    heapsort(&mut array.view_mut(), &mut |x: &usize, y: &usize| x.cmp(&y));

    for window in array.data.windows(2) {
        assert!(window[0] < window[1]);
    }
    for i in 0..20 {
        let mut key = crate::entity::Key::zero();
        key.set_index(i);
        assert_eq!(array.get(key), Some(&i));
    }
    for i in 20..100 {
        let mut key = crate::entity::Key::zero();
        key.set_index(100 - i + 20);
        assert_eq!(array.get(key), Some(&i));
    }
}
