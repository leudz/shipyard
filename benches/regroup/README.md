# Targeted regroup benchmarks

Run from the checkout root. Build, smoke-test (one invocation of each case with its checks),
then measure using Criterion's default configuration:

```sh
cargo bench --bench regroup --no-run
cargo bench --bench regroup -- --test
cargo bench --bench regroup
```

There are exactly eight cases, extracted from the regroup follow-up experiments:

| Case | Timed work |
| --- | --- |
| `spawn_ab_bulk/shipyard_grouped` | Bulk-create 100,000 AB entities and regroup. |
| `delete_ab/shipyard_grouped` | Delete 10,000 scattered entities from 100,000 AB entities and regroup. |
| `add_b/shipyard_grouped` | Restore B to 10,000 scattered A-only entities among 100,000 entities and regroup. |
| `remove_b/shipyard_grouped` | Remove B from 10,000 scattered entities among 100,000 AB entities and regroup. |
| `cross_add_a/shipyard_grouped` | Restore A to 2,500 entities in each original AB, AC and ABC quarter and regroup. |
| `cross_remove_a/shipyard_grouped` | Remove A from 2,500 entities in each AB, AC and ABC quarter and regroup. |
| `delete_ab/shipyard_current` | Delete the same population as grouped deletion, in an ungrouped world. |
| `iterate_ab_full/shipyard_current` | Acquire views and mutate A joined with shared B over 100,000 AB entities, alongside 100,000 entities with neither payload. |

Structural cases use `iter_custom` with a fresh world per invocation. World construction,
population setup, deterministic selection, initial regroups, preparation, validation and world
destruction are outside the timer. The timed work includes view acquisition, mutation, view
release and, for grouped cases, the final regroup. The bulk-spawn input vector is built once
outside timing; its `input.iter().copied()` iterator is created and consumed inside timing.
The previous world is dropped before constructing the next one, and the final world is checked
after Criterion finishes the case. Filtered-out structural cases have no final world to check.

The iteration control builds and prepares its world before timing by removing and re-adding A
for 10,000 shuffled holders. Each timed traversal acquires its views and updates all four numeric
fields of A with wrapping additions from B. The post-timing checks use the total traversal count
across all Criterion callbacks and verify the untouched payload fields as well.

A, B and C retain their 64-byte payloads and original initialization. Basic structural selections
use seed `0x5EED_0002`, and iteration preparation uses `0x5EED_0001`. Crossing worlds declare AB
and AC and contain 25,000 entities each of A, AB, AC and ABC. Their selections shuffle the three
affected quarters with `0x5EED_0004`, seed + 1 and seed + 2 and interleave AB, AC and ABC selections.
Checks verify join membership and uniqueness, payload preservation, component presence or absence,
and entity survival or deletion as applicable.
