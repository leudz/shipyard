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
    for i in 2..=view.dense.len() {
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

#[test]
fn insert_sort() {
    let mut array = crate::sparse_array::SparseArray::default();

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
