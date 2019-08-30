use crate::sparse_array::ViewMut;
use std::cmp::Ordering;
use std::ptr;

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

    if i * 2 < len {
        i = i * 2 + 1;
    }

    i
}

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
}

#[allow(dead_code)]
fn heapsort<T, F: FnMut(&T, &T) -> Ordering>(view: &mut ViewMut<T>, cmp: &mut F) {
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
fn heap_sort() {
    let mut array = crate::sparse_array::SparseArray::default();

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
    let mut array = crate::sparse_array::SparseArray::default();

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
