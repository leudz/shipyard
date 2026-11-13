//! Payloads, worlds and deterministic selection from the regroup experiments.

use shipyard::{Component, World, track};

/// 64-byte payload, the component every query updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct A {
    pub x: u64,
    pub y: u64,
    pub z: u64,
    pub w: u64,
    pub untouched: [u64; 4],
}

/// 64-byte payload, read by the `(A, B)` joins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct B {
    pub x: u64,
    pub y: u64,
    pub z: u64,
    pub w: u64,
    pub untouched: [u64; 4],
}

/// 64-byte payload, read by the `(A, C)` joins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct C {
    pub x: u64,
    pub y: u64,
    pub z: u64,
    pub w: u64,
    pub untouched: [u64; 4],
}

impl A {
    /// Starting value of logical entity `entity`.
    pub fn new(entity: u32) -> A {
        let entity = u64::from(entity);

        A {
            x: entity,
            y: entity + 1,
            z: entity + 2,
            w: entity + 3,
            untouched: [!entity; 4],
        }
    }
}

impl B {
    /// Starting value of logical entity `entity`, different from `A`'s and `C`'s.
    pub fn new(entity: u32) -> B {
        let entity = 10 * u64::from(entity) + 1;

        B {
            x: entity,
            y: entity + 1,
            z: entity + 2,
            w: entity + 3,
            untouched: [!entity; 4],
        }
    }
}

impl C {
    /// Starting value of logical entity `entity`, different from `A`'s and `B`'s.
    pub fn new(entity: u32) -> C {
        let entity = 100 * u64::from(entity) + 2;

        C {
            x: entity,
            y: entity + 1,
            z: entity + 2,
            w: entity + 3,
            untouched: [!entity; 4],
        }
    }
}

const _: () = assert!(size_of::<A>() == 64);
const _: () = assert!(size_of::<B>() == 64);
const _: () = assert!(size_of::<C>() == 64);

impl Component for A {
    type Tracking = track::Untracked;
}

impl Component for B {
    type Tracking = track::Untracked;
}

impl Component for C {
    type Tracking = track::Untracked;
}

/// Empty `shipyard_grouped` world for the basic scenarios, with `(A, B)` grouped.
pub fn grouped_world() -> World {
    World::builder().with_group::<(A, B)>().build()
}

/// Empty `shipyard_grouped` world for the crossing scenarios, with `(A, B)` and `(A, C)` grouped.
pub fn crossing_grouped_world() -> World {
    World::builder()
        .with_group::<(A, B)>()
        .with_group::<(A, C)>()
        .build()
}

/// Returns `items` in a fixed pseudo-random order picked by `seed`.
///
/// Benchmarks select scattered logical entities by taking a prefix of the result.
/// The same `items` and `seed` always give the same order, so both trees select the same
/// logical entities, in the same order.
pub fn shuffled(mut items: Vec<u32>, seed: u64) -> Vec<u32> {
    // Fisher-Yates, driven by a splitmix64 counter.
    let mut counter = seed;
    for last in (1..items.len()).rev() {
        counter = counter.wrapping_add(1);
        let other = (splitmix64(counter) % (last as u64 + 1)) as usize;
        items.swap(last, other);
    }

    items
}

fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    state = (state ^ (state >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    state ^ (state >> 31)
}
