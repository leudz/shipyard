# Tracking

Shipyard comes with built-in tracking for *insertion*, *modification*, *deletion* and *removal*.\
*deletion* will store the component in the tracking info whereas *removal* gives it back immediately.\
It can be noticed on `SparseSet::delete` vs `SparseSet::remove` signatures:

```rs
fn delete(&mut self, entity: EntityId) -> bool {}
fn remove(&mut self, entity: EntityId) -> Option<T> {}
```

Components can be deleted or removed but whole entities can only be deleted (at least for now, it's technically possible to return something but I digress).

## Declaration

Tracking is set with the `Component` trait. You can set it to any single operation or use `All` to track everything.

```rs
{{#include ../../../../tests/book/tracking.rs:component}}
{{#include ../../../../tests/book/tracking.rs:component_proc}}
```

## Usage

When inside a workload you will get all tracking information since the last time this system ran.\
Outside workloads you'll get information since the last call to `clear_*`.

#### *Inserted* or *Modified*

You can query *inserted* and *modified* components when iterating by calling `inserted`, `modified` or `inserted_or_modified` on a view before making the iterator. (*_mut versions also exist).

```rs
{{#include ../../../../tests/book/tracking.rs:run}}
```

#### *Removed* or *Deleted*

*Removed* and *deleted* cannot be used with `iter` but can be accessed with `removed`, `deleted` or `removed_or_deleted` methods on views.

## Reset

Inside workloads tracking information doesn't need to be reset. You will always get the operations that happened since the last run of the system.

You can reset out of workload tracking info with:
- `clear_all_inserted`
- `clear_all_modified`
- `clear_all_inserted_and_modified`
- `clear_all_removed`
- `clear_all_deleted`
- `clear_all_removed_and_deleted`

You can also reset removed and deleted information older than some timestamp.

Use `World::get_tracking_timestamp` or `AllStorages::get_tracking_timestamp` to get a timestamp.\
Then call `clear_all_deleted_older_than_timestamp`, `clear_all_removed_older_than_timestamp` or `clear_all_removed_and_deleted_older_than_timestamp`.
