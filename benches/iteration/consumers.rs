//! The timed bodies.
//!
//! Each consumer is `#[inline(never)]` so its monomorphizations show up by name in the
//! emitted assembly. They're generic over their iterator, so fixtures yielding the same
//! iterator type share the same machine code. The arithmetic stays in the loop body or the
//! closure, nothing is called per element.
//!
//! The read expression is the same everywhere: `acc.wrapping_add(a ^ (b << 3))`, with
//! `^ (c << 7)` for triples and `id_hash ^` for IDs. Wrapping addition doesn't care about
//! iteration order, which isn't stable. `Fixture::pair_sum` and friends recompute these
//! from the inserted values.

use crate::fixtures::{A, B, C};
use shipyard::{EntityId, IntoIter, View};
use std::hint::black_box;

fn id_hash(id: EntityId) -> u32 {
    (id.index() as u32).wrapping_mul(0x9E37_79B9)
}

/// Reads pairs through `for`, so through `next`.
#[inline(never)]
pub fn next_pair<'a, I: Iterator<Item = (&'a A, &'a B)>>(iter: I) -> u32 {
    let mut acc = 0u32;
    for (a, b) in iter {
        acc = acc.wrapping_add(a.0 ^ (b.0 << 3));
    }
    acc
}

/// Writes `mutated_a(b)` to A through `for`.
#[inline(never)]
pub fn next_pair_mut<'a, I: Iterator<Item = (&'a mut A, &'a B)>>(iter: I) {
    for (a, b) in iter {
        a.0 = b.0.wrapping_mul(3) ^ 0xA5A5;
    }
}

/// Reads pairs and their IDs through `for`.
#[inline(never)]
pub fn next_pair_ids<'a, I: Iterator<Item = (EntityId, (&'a A, &'a B))>>(iter: I) -> u32 {
    let mut acc = 0u32;
    for (id, (a, b)) in iter {
        acc = acc.wrapping_add(id_hash(id) ^ a.0 ^ (b.0 << 3));
    }
    acc
}

/// Reads flat triples through `for`.
#[inline(never)]
pub fn next_triple<'a, I: Iterator<Item = (&'a A, &'a B, &'a C)>>(iter: I) -> u32 {
    let mut acc = 0u32;
    for (a, b, c) in iter {
        acc = acc.wrapping_add(a.0 ^ (b.0 << 3) ^ (c.0 << 7));
    }
    acc
}

/// Reads nested triples through `for`.
#[inline(never)]
pub fn next_nested<'a, I: Iterator<Item = ((&'a A, &'a B), &'a C)>>(iter: I) -> u32 {
    let mut acc = 0u32;
    for ((a, b), c) in iter {
        acc = acc.wrapping_add(a.0 ^ (b.0 << 3) ^ (c.0 << 7));
    }
    acc
}

/// Reads pairs through `fold`.
#[inline(never)]
pub fn fold_pair<'a, I: Iterator<Item = (&'a A, &'a B)>>(iter: I) -> u32 {
    iter.fold(0u32, |acc, (a, b)| acc.wrapping_add(a.0 ^ (b.0 << 3)))
}

/// Writes `mutated_a(b)` to A through `fold`. Each write only depends on its own B.
#[inline(never)]
pub fn fold_pair_mut<'a, I: Iterator<Item = (&'a mut A, &'a B)>>(iter: I) {
    iter.fold((), |(), (a, b)| a.0 = b.0.wrapping_mul(3) ^ 0xA5A5);
}

/// Reads pairs and their IDs through `fold`.
#[inline(never)]
pub fn fold_pair_ids<'a, I: Iterator<Item = (EntityId, (&'a A, &'a B))>>(iter: I) -> u32 {
    iter.fold(0u32, |acc, (id, (a, b))| {
        acc.wrapping_add(id_hash(id) ^ a.0 ^ (b.0 << 3))
    })
}

/// Reads pairs through `for_each`, into an accumulator the closure captures.
#[inline(never)]
pub fn for_each_pair<'a, I: Iterator<Item = (&'a A, &'a B)>>(iter: I) -> u32 {
    let mut acc = 0u32;
    iter.for_each(|(a, b)| acc = acc.wrapping_add(a.0 ^ (b.0 << 3)));
    acc
}

/// Reads flat triples through `fold`.
#[inline(never)]
pub fn fold_triple<'a, I: Iterator<Item = (&'a A, &'a B, &'a C)>>(iter: I) -> u32 {
    iter.fold(0u32, |acc, (a, b, c)| {
        acc.wrapping_add(a.0 ^ (b.0 << 3) ^ (c.0 << 7))
    })
}

/// Builds the pair iterator, reads it through `for` and drops it.
#[inline(never)]
pub fn construct_next_pair(a: &View<'_, A>, b: &View<'_, B>) -> u32 {
    let mut acc = 0u32;
    for (a, b) in (a, b).iter() {
        acc = acc.wrapping_add(a.0 ^ (b.0 << 3));
    }
    acc
}

/// Builds the pair iterator, reads it through `fold` and drops it.
#[inline(never)]
pub fn construct_fold_pair(a: &View<'_, A>, b: &View<'_, B>) -> u32 {
    (a, b)
        .iter()
        .fold(0u32, |acc, (a, b)| acc.wrapping_add(a.0 ^ (b.0 << 3)))
}

/// Builds the pair iterator and drops it without advancing it.
#[inline(never)]
pub fn construct_drop_pair(a: &View<'_, A>, b: &View<'_, B>) {
    let iter = (a, b).iter();
    // We hide the iterator so its construction can't be optimized out, then drop it untouched.
    drop(black_box(iter));
}
