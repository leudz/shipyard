//! Consumes iterators in every way and checks they agree.

/// Consumes the iterators `$iter` builds in every way and asserts they all yield
/// the same items in the same order, then does it again after every `next` prefix length.
///
/// `$iter` is evaluated anew for each traversal so it can borrow views mutably.
/// `$value` turns an item into a comparable value, closures need their parameter typed.\
/// Returns the items with their ids, as `WithId::next` yields them.
macro_rules! every_way {
    ($iter: expr, $value: expr $(,)?) => {{
        let value = $value;

        // `for` loops only call `next`.
        let mut with_ids = Vec::new();
        for (id, item) in $iter.with_id() {
            with_ids.push((id, value(item)));
        }
        let ids: Vec<_> = with_ids.iter().map(|(id, _)| *id).collect();
        let values: Vec<_> = with_ids.iter().map(|(_, value)| value.clone()).collect();

        let mut next = Vec::new();
        for item in $iter {
            next.push(value(item));
        }
        assert_eq!(next, values, "next");

        let fold = $iter.fold(Vec::new(), |mut acc, item| {
            acc.push(value(item));
            acc
        });
        assert_eq!(fold, values, "fold");

        let mut for_each = Vec::new();
        $iter.for_each(|item| for_each.push(value(item)));
        assert_eq!(for_each, values, "for_each");

        let id_fold = $iter.with_id().fold(Vec::new(), |mut acc, (id, item)| {
            acc.push((id, value(item)));
            acc
        });
        assert_eq!(id_fold, with_ids, "WithId fold");

        let mut id_for_each = Vec::new();
        $iter
            .with_id()
            .for_each(|(id, item)| id_for_each.push((id, value(item))));
        assert_eq!(id_for_each, with_ids, "WithId for_each");

        let mut only_ids = Vec::new();
        $iter.ids().for_each(|id| only_ids.push(id));
        assert_eq!(only_ids, ids, "ids");

        // Bulk consumption resumes where `next` stopped, before, inside, at the end
        // and past the end of the items.
        for prefix in 0..=with_ids.len() + 1 {
            let mut iter = $iter;
            let mut head = Vec::new();
            for item in (&mut iter).take(prefix) {
                head.push(value(item));
            }
            let fold = iter.fold(head, |mut acc, item| {
                acc.push(value(item));
                acc
            });
            assert_eq!(fold, values, "fold after {prefix}");

            let mut iter = $iter;
            let mut for_each = Vec::new();
            for item in (&mut iter).take(prefix) {
                for_each.push(value(item));
            }
            iter.for_each(|item| for_each.push(value(item)));
            assert_eq!(for_each, values, "for_each after {prefix}");

            let mut iter = $iter.with_id();
            let mut head = Vec::new();
            for (id, item) in (&mut iter).take(prefix) {
                head.push((id, value(item)));
            }
            let id_fold = iter.fold(head, |mut acc, (id, item)| {
                acc.push((id, value(item)));
                acc
            });
            assert_eq!(id_fold, with_ids, "WithId fold after {prefix}");

            let mut iter = $iter.with_id();
            let mut id_for_each = Vec::new();
            for (id, item) in (&mut iter).take(prefix) {
                id_for_each.push((id, value(item)));
            }
            iter.for_each(|(id, item)| id_for_each.push((id, value(item))));
            assert_eq!(id_for_each, with_ids, "WithId for_each after {prefix}");
        }

        with_ids
    }};
}

pub(crate) use every_way;
