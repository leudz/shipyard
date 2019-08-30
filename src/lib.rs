//! # Getting started
//! ```
//! use shipyard::*;
//!
//! struct Health(f32);
//! struct Position { x: f32, y: f32 };
//!
//! #[system(InAcid)]
//! fn run(pos: &Position, mut health: &mut Health) {
//!     for (pos, health) in (pos, health).iter() {
//!         if is_in_acid(pos) {
//!             health.0 -= 1.0;
//!         }
//!     }
//! }
//!
//! fn is_in_acid(pos: &Position) -> bool {
//!     // well... it's wet season
//!      
//!     true
//! }
//!
//! let world = World::new::<(Position, Health)>();
//!
//! world.run::<(EntitiesMut, &mut Position, &mut Health), _>(|(mut entities, mut pos, mut health)| {
//!     entities.add_entity((&mut pos, &mut health), (Position { x: 0.0, y: 0.0 }, Health(1000.0)));
//! });
//!
//! world.add_workload("In acid", InAcid);
//! world.run_default();
//! ```
//! # Let's make some pigs!
//! ```
//! # #[cfg(feature = "parallel")]
//! # {
//! use shipyard::*;
//!
//! struct Health(f32);
//! struct Fat(f32);
//!
//! #[system(Reproduction)]
//! fn run(mut fat: &mut Fat, mut health: &mut Health, mut entities: &mut Entities) {
//!     let count = (&health, &fat).iter().filter(|(health, fat)| health.0 > 40.0 && fat.0 > 20.0).count();
//!     (0..count).for_each(|_| {
//!         entities.add_entity((&mut health, &mut fat), (Health(100.0), Fat(0.0)));
//!     });
//! }
//!
//! #[system(Meal)]
//! fn run(fat: &mut Fat) {
//!     for slice in fat.iter().into_chunk(8) {
//!         for fat in slice {
//!             fat.0 += 3.0;
//!         }
//!     }
//! }
//!
//! #[system(Age)]
//! fn run(health: &mut Health, thread_pool: ThreadPool) {
//!     use rayon::prelude::ParallelIterator;
//!
//!     thread_pool.install(|| {
//!         health.par_iter().for_each(|health| {
//!             health.0 -= 4.0;
//!         });
//!     });
//! }
//!
//! let world = World::new::<(Health, Fat)>();
//!
//! world.run::<(EntitiesMut, &mut Health, &mut Fat), _>(|(mut entities, mut health, mut fat)| {
//!     (0..100).for_each(|_| {
//!         entities.add_entity((&mut health, &mut fat), (Health(100.0), Fat(0.0)));
//!     })
//! });
//!
//! world.add_workload("Life", (Meal, Age));
//! world.add_workload("Reproduction", Reproduction);
//!
//! for day in 0..100 {
//!     if day % 6 == 0 {
//!         world.run_workload("Reproduction");
//!     }
//!     world.run_default();
//! }
//!
//! world.run::<&Health, _>(|health| {
//!     // we've got some new pigs
//!     assert_eq!(health.len(), 900);
//! });
//! # }
//! ```

#![deny(bare_trait_objects)]

mod atomic_refcell;
mod component_storage;
mod entity;
pub mod error;
mod get;
pub mod iterators;
mod not;
mod remove;
mod run;
mod sparse_array;
mod unknown_storage;
mod world;

pub use crate::component_storage::AllStorages;
pub use crate::get::GetComponent;
pub use crate::not::Not;
pub use crate::remove::Remove;
pub use crate::run::System;
#[doc(hidden)]
pub use crate::run::SystemData;
pub use crate::world::World;
pub use entity::{Entities, EntitiesMut, EntitiesViewMut, Key};
pub use iterators::IntoIter;
#[doc(hidden)]
#[cfg(feature = "proc")]
pub use shipyard_proc::system;
pub use sparse_array::{sort, sort::Sortable};

/// Type used to borrow the rayon::ThreadPool inside `World`.
#[cfg(feature = "parallel")]
pub struct ThreadPool;

#[test]
fn add_entity() {
    let world = World::default();
    world.register::<usize>();
    world.register::<u32>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
    });
}

#[test]
fn tight_add_entity() {
    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
    });
}

#[test]
fn loose_add_entity() {
    let world = World::new::<(usize, u32)>();
    world.loose_pack::<(usize,), (u32,)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
    });
}

#[test]
fn tight_loose_add_entity() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();

    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _>(
        |(mut entities, mut usizes, mut u64s, mut u32s)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
            assert_eq!((&usizes, &u64s, &u32s).get(entity1).unwrap(), (&0, &1, &2));
        },
    );
}

#[test]
fn add_component() {
    let world = World::new::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        let entity1 = entities.add_entity((), ());
        entities.add_component((&mut usizes, &mut u32s), (0, 1), entity1);
        entities.add_component((&mut u32s, &mut usizes), (3, 2), entity1);
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
    });
}

#[test]
fn tight_add_component() {
    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        let entity1 = entities.add_entity((), ());
        entities.add_component((&mut usizes, &mut u32s), (0, 1), entity1);
        entities.add_component((&mut u32s, &mut usizes), (3, 2), entity1);
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
        let mut iter = (&usizes, &u32s).iter();
        assert_eq!(iter.next(), Some((&2, &3)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn loose_add_component() {
    let world = World::new::<(usize, u32)>();
    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        let entity1 = entities.add_entity((), ());
        entities.add_component((&mut usizes, &mut u32s), (0, 1), entity1);
        entities.add_component((&mut u32s, &mut usizes), (3, 2), entity1);
        assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
        let mut iter = (&usizes, &u32s).iter();
        assert_eq!(iter.next(), Some((&2, &3)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn tight_loose_add_component() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();
    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _>(
        |(mut entities, mut usizes, mut u64s, mut u32s)| {
            let entity1 = entities.add_entity((), ());
            entities.add_component((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2), entity1);
            entities.add_component((&mut u32s, &mut u64s, &mut usizes), (5, 4, 3), entity1);
            assert_eq!((&usizes, &u64s, &u32s).get(entity1).unwrap(), (&3, &4, &5));
            let mut iter = (&usizes, &u32s, &u64s).iter();
            assert_eq!(iter.next(), Some((&3, &5, &4)));
            assert_eq!(iter.next(), None);
            let mut iter = (&usizes, &u64s).iter();
            assert_eq!(iter.next(), Some((&3, &4)));
            assert_eq!(iter.next(), None);
        },
    );
}

#[test]
fn add_additional_component() {
    let world = World::new::<(usize, u32, String)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32, &mut String), _>(
        |(mut entities, mut usizes, mut u32s, mut strings)| {
            let entity1 = entities.add_entity((), ());
            entities.add_component((&mut usizes, &mut u32s, &mut strings), (0, 1), entity1);
            entities.add_component((&mut usizes, &mut u32s, &mut strings), (2, 3), entity1);
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
        },
    );
}

#[test]
fn tight_add_additional_component() {
    let world = World::new::<(usize, u32, String)>();
    world.tight_pack::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32, &mut String), _>(
        |(mut entities, mut usizes, mut u32s, mut strings)| {
            let entity1 = entities.add_entity((), ());
            entities.add_component((&mut usizes, &mut u32s, &mut strings), (0, 1), entity1);
            entities.add_component((&mut usizes, &mut u32s, &mut strings), (2, 3), entity1);
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
            let mut iter = (&usizes, &u32s).iter();
            assert_eq!(iter.next(), Some((&2, &3)));
            assert_eq!(iter.next(), None);
        },
    );
}

#[test]
fn loose_add_additional_component() {
    let world = World::new::<(usize, u32, String)>();
    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32, &mut String), _>(
        |(mut entities, mut usizes, mut u32s, mut strings)| {
            let entity1 = entities.add_entity((), ());
            entities.add_component((&mut usizes, &mut u32s, &mut strings), (0, 1), entity1);
            entities.add_component((&mut usizes, &mut u32s, &mut strings), (2, 3), entity1);
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
            let mut iter = (&usizes, &u32s).iter();
            assert_eq!(iter.next(), Some((&2, &3)));
            assert_eq!(iter.next(), None);
        },
    );
}

#[test]
fn tight_loose_add_additional_component() {
    let world = World::new::<(usize, u64, u32, String)>();
    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();
    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32, &mut String), _>(
        |(mut entities, mut usizes, mut u64s, mut u32s, mut strings)| {
            let entity1 = entities.add_entity((), ());
            entities.add_component(
                (&mut usizes, &mut u64s, &mut u32s, &mut strings),
                (0, 1, 2),
                entity1,
            );
            entities.add_component(
                (&mut usizes, &mut u64s, &mut u32s, &mut strings),
                (3, 4, 5),
                entity1,
            );
            assert_eq!((&usizes, &u64s, &u32s).get(entity1).unwrap(), (&3, &4, &5));
            let mut iter = (&usizes, &u32s, &u64s).iter();
            assert_eq!(iter.next(), Some((&3, &5, &4)));
            assert_eq!(iter.next(), None);
            let mut iter = (&usizes, &u64s).iter();
            assert_eq!(iter.next(), Some((&3, &4)));
            assert_eq!(iter.next(), None);
        },
    );
}

#[test]
fn run() {
    let world = World::new::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));

        // possible to borrow twice as immutable
        let mut iter1 = (&usizes).iter();
        let _iter2 = (&usizes).iter();
        assert_eq!(iter1.next(), Some(&0));

        // impossible to borrow twice as mutable
        // if switched, the next two lines should trigger an error
        let _iter = (&mut usizes).iter();
        let mut iter = (&mut usizes).iter();
        assert_eq!(iter.next(), Some(&mut 0));
        assert_eq!(iter.next(), Some(&mut 2));
        assert_eq!(iter.next(), None);

        // possible to borrow twice as immutable
        let mut iter = (&usizes, &u32s).iter();
        let _iter = (&usizes, &u32s).iter();
        assert_eq!(iter.next(), Some((&0, &1)));
        assert_eq!(iter.next(), Some((&2, &3)));
        assert_eq!(iter.next(), None);

        // impossible to borrow twice as mutable
        // if switched, the next two lines should trigger an error
        let _iter = (&mut usizes, &u32s).iter();
        let mut iter = (&mut usizes, &u32s).iter();
        assert_eq!(iter.next(), Some((&mut 0, &1)));
        assert_eq!(iter.next(), Some((&mut 2, &3)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn iterators() {
    let world = World::new::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        let entity1 = entities.add_entity((&mut usizes,), (0usize,));
        entities.add_component((&mut u32s,), (1u32,), entity1);
        entities.add_entity((&mut usizes,), (2usize,));

        let mut iter = (&usizes).iter();
        assert_eq!(iter.next(), Some(&0));
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.next(), None);

        let mut iter = (&usizes, &u32s).iter();
        assert_eq!(iter.next(), Some((&0, &1)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn not_iterators() {
    let world = World::new::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes,), (2usize,));

        let not_usizes = !usizes;
        let mut iter = (&not_usizes).iter();
        assert_eq!(iter.next(), None);

        let mut iter = (&not_usizes, !&u32s).iter();
        assert_eq!(iter.next(), None);

        let mut iter = (&not_usizes, &u32s).iter();
        assert_eq!(iter.next(), None);

        let usizes = not_usizes.into_inner();

        let mut iter = (&usizes, !&u32s).iter();
        assert_eq!(iter.next(), Some((&2, ())));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn pack_missing_storage() {
    match std::panic::catch_unwind(|| {
        let world = World::new::<(usize, u32)>();
        world.tight_pack::<(usize, u32)>();

        world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
            let entity = entities.add_entity((), ());
            entities.add_component((&mut usizes, &mut u32s), (0, 1), entity);
            entities.add_component((&mut usizes, &mut u32s), (2,), entity);
            entities.add_component(&mut usizes, 0, entity);
        });
    }) {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(format!("{}", err.downcast::<String>().unwrap()), format!("called `Result::unwrap()` on an `Err` value: Missing storage for type ({:?}). To add a packed component you have to pass all storages packed with it. Even if you just add one component.", std::any::TypeId::of::<usize>())),
    }
}

#[test]
fn tight_iterator() {
    use iterators::Iter2;

    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes,), (2usize,));
        // test for consuming version
        entities.add_entity((usizes, u32s), (3usize, 4u32));
    });

    world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
        if let Iter2::Tight(mut iter) = (&usizes, &u32s).iter() {
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), Some((&3, &4)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
    });
}

#[test]
fn post_tight_iterator() {
    use iterators::Iter2;

    let world = World::new::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes,), (2usize,));
        // test for consuming version
        entities.add_entity((usizes, u32s), (3usize, 4u32));
    });

    world.tight_pack::<(usize, u32)>();

    world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
        if let Iter2::Tight(mut iter) = (&usizes, &u32s).iter() {
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), Some((&3, &4)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
    });
}

#[test]
fn chunk_iterator() {
    use iterators::Iter2;

    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        entities.add_entity((&mut usizes, &mut u32s), (4usize, 5u32));
        entities.add_entity((&mut usizes, &mut u32s), (6usize, 7u32));
        entities.add_entity((&mut usizes, &mut u32s), (8usize, 9u32));
        entities.add_entity((&mut usizes,), (10usize,));
    });

    world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
        if let Iter2::Tight(iter) = (&usizes, &u32s).iter() {
            let mut iter = iter.into_chunk(2);
            assert_eq!(iter.next(), Some((&[0, 2][..], &[1, 3][..])));
            assert_eq!(iter.next(), Some((&[4, 6][..], &[5, 7][..])));
            assert_eq!(iter.next(), Some((&[8][..], &[9][..])));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
        if let Iter2::Tight(iter) = (&usizes, &u32s).iter() {
            let mut iter = iter.into_chunk_exact(2);
            assert_eq!(iter.next(), Some((&[0, 2][..], &[1, 3][..])));
            assert_eq!(iter.next(), Some((&[4, 6][..], &[5, 7][..])));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.remainder(), (&[8][..], &[9][..]));
            assert_eq!(iter.remainder(), (&[][..], &[][..]));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
        if let Iter2::Tight(mut iter) = (&usizes, &u32s).iter() {
            iter.next();
            let mut iter = iter.into_chunk(2);
            assert_eq!(iter.next(), Some((&[2, 4][..], &[3, 5][..])));
            assert_eq!(iter.next(), Some((&[6, 8][..], &[7, 9][..])));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
        if let Iter2::Tight(mut iter) = (&usizes, &u32s).iter() {
            iter.next();
            let mut iter = iter.into_chunk_exact(2);
            assert_eq!(iter.next(), Some((&[2, 4][..], &[3, 5][..])));
            assert_eq!(iter.next(), Some((&[6, 8][..], &[7, 9][..])));
            assert_eq!(iter.remainder(), (&[][..], &[][..]));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
        if let Iter2::Tight(mut iter) = (&usizes, &u32s).iter() {
            iter.next();
            iter.next();
            let mut iter = iter.into_chunk_exact(2);
            assert_eq!(iter.next(), Some((&[4, 6][..], &[5, 7][..])));
            assert_eq!(iter.next(), None);
            assert_eq!(iter.remainder(), (&[8][..], &[9][..]));
            assert_eq!(iter.remainder(), (&[][..], &[][..]));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
        if let Iter2::Tight(mut iter) = (&usizes, &u32s).iter() {
            iter.nth(3);
            let mut iter = iter.into_chunk_exact(2);
            assert_eq!(iter.next(), None);
            assert_eq!(iter.remainder(), (&[8][..], &[9][..]));
            assert_eq!(iter.remainder(), (&[][..], &[][..]));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
    });
}

#[test]
fn loose_iterator() {
    use iterators::Iter2;

    let world = World::new::<(usize, u32)>();
    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes,), (2usize,));
        // test for consuming version
        entities.add_entity((usizes, u32s), (3usize, 4u32));
    });

    world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
        if let Iter2::Loose(mut iter) = (&usizes, &u32s).iter() {
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), Some((&3, &4)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
    });
}

#[test]
fn post_loose_iterator() {
    use iterators::Iter2;

    let world = World::new::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes,), (2usize,));
        // test for consuming version
        entities.add_entity((usizes, u32s), (3usize, 4u32));
    });

    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
        if let Iter2::Loose(mut iter) = (&usizes, &u32s).iter() {
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), Some((&3, &4)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
    });
}

#[test]
fn tight_loose_iterator() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();

    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _>(
        |(mut entities, mut usizes, mut u64s, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
            entities.add_entity((&mut usizes, &mut u64s), (3, 4));
            entities.add_entity((&mut usizes,), (5,));
            // test for consuming version
            entities.add_entity((usizes, u64s, u32s), (6, 7, 8));
        },
    );

    world.run::<(&usize, &u64, &u32), _>(|(usizes, u64s, u32s)| {
        if let iterators::Iter3::Loose(mut iter) = (&usizes, &u64s, &u32s).iter() {
            assert_eq!(iter.next(), Some((&0, &1, &2)));
            assert_eq!(iter.next(), Some((&6, &7, &8)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not loose");
        }
        if let iterators::Iter2::Tight(mut iter) = (&usizes, &u64s).iter() {
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), Some((&3, &4)));
            assert_eq!(iter.next(), Some((&6, &7)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not tight");
        }
        if let iterators::Iter2::NonPacked(mut iter) = (&usizes, &u32s).iter() {
            assert_eq!(iter.next(), Some((&0, &2)));
            assert_eq!(iter.next(), Some((&6, &8)));
            assert_eq!(iter.next(), None);
        }
    });
}

#[test]
fn remove() {
    let world = World::new::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        let component = Remove::<(usize,)>::remove((&mut usizes,), entity1);
        assert_eq!(component, (Some(0usize),));
        assert_eq!((&mut usizes).get(entity1), None);
        assert_eq!((&mut u32s).get(entity1), Some(&mut 1));
        assert_eq!(usizes.get(entity2), Some(&2));
        assert_eq!(u32s.get(entity2), Some(&3));
    });
}

#[test]
fn remove_tight() {
    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        let component = Remove::<(usize,)>::remove((&mut usizes, &mut u32s), entity1);
        assert_eq!(component, (Some(0usize),));
        assert_eq!((&mut usizes).get(entity1), None);
        assert_eq!((&mut u32s).get(entity1), Some(&mut 1));
        assert_eq!(usizes.get(entity2), Some(&2));
        assert_eq!(u32s.get(entity2), Some(&3));
        let iter = (&usizes, &u32s).iter();
        if let iterators::Iter2::Tight(mut iter) = iter {
            assert_eq!(iter.next(), Some((&2, &3)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }
    });
}

#[test]
fn remove_loose() {
    let world = World::new::<(usize, u32)>();
    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        let component = Remove::<(usize,)>::remove((&mut usizes, &mut u32s), entity1);
        assert_eq!(component, (Some(0usize),));
        assert_eq!((&mut usizes).get(entity1), None);
        assert_eq!((&mut u32s).get(entity1), Some(&mut 1));
        assert_eq!(usizes.get(entity2), Some(&2));
        assert_eq!(u32s.get(entity2), Some(&3));
        let mut iter = (&usizes, &u32s).iter();
        assert_eq!(iter.next(), Some((&2, &3)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn remove_tight_loose() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();

    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _>(
        |(mut entities, mut usizes, mut u64s, mut u32s)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
            let entity2 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (3, 4, 5));
            entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (6, 7, 8));
            let component = Remove::<(u32,)>::remove((&mut u32s, &mut usizes, &mut u64s), entity1);
            assert_eq!(component, (Some(2),));
            let mut iter = (&usizes, &u64s).iter();
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), Some((&3, &4)));
            assert_eq!(iter.next(), Some((&6, &7)));
            assert_eq!(iter.next(), None);
            let iter = (&usizes, &u64s, &u32s).iter();
            if let iterators::Iter3::Loose(mut iter) = iter {
                assert_eq!(iter.next(), Some((&6, &7, &8)));
                assert_eq!(iter.next(), Some((&3, &4, &5)));
                assert_eq!(iter.next(), None);
            }
            let component =
                Remove::<(usize,)>::remove((&mut usizes, &mut u32s, &mut u64s), entity2);
            assert_eq!(component, (Some(3),));
            let mut iter = (&usizes, &u64s).iter();
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), Some((&6, &7)));
            assert_eq!(iter.next(), None);
            let mut iter = (&usizes, &u64s, &u32s).iter();
            assert_eq!(iter.next(), Some((&6, &7, &8)));
            assert_eq!(iter.next(), None);
        },
    );
}

#[test]
fn delete() {
    let world = World::new::<(usize, u32)>();

    let mut entity1 = None;
    let mut entity2 = None;
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entity1 = Some(entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)));
        entity2 = Some(entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)));
    });

    world.run::<(EntitiesMut, AllStorages), _>(|(mut entities, mut all_storages)| {
        assert!(entities.delete(&mut all_storages, entity1.unwrap()));
        assert!(!entities.delete(&mut all_storages, entity1.unwrap()));
    });

    world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
        assert_eq!((&usizes).get(entity1.unwrap()), None);
        assert_eq!((&u32s).get(entity1.unwrap()), None);
        assert_eq!(usizes.get(entity2.unwrap()), Some(&2));
        assert_eq!(u32s.get(entity2.unwrap()), Some(&3));
    });
}

#[test]
fn delete_tight() {
    let world = World::new::<(usize, u32)>();

    world.tight_pack::<(usize, u32)>();

    let mut entity1 = None;
    let mut entity2 = None;
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entity1 = Some(entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)));
        entity2 = Some(entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)));
    });

    world.run::<(EntitiesMut, AllStorages), _>(|(mut entities, mut all_storages)| {
        assert!(entities.delete(&mut all_storages, entity1.unwrap()));
        assert!(!entities.delete(&mut all_storages, entity1.unwrap()));
    });

    world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
        assert_eq!((&usizes).get(entity1.unwrap()), None);
        assert_eq!((&u32s).get(entity1.unwrap()), None);
        assert_eq!(usizes.get(entity2.unwrap()), Some(&2));
        assert_eq!(u32s.get(entity2.unwrap()), Some(&3));
        let mut iter = (&usizes, &u32s).iter();
        assert_eq!(iter.next(), Some((&2, &3)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn delete_loose() {
    let world = World::new::<(usize, u32)>();

    world.loose_pack::<(usize,), (u32,)>();

    let mut entity1 = None;
    let mut entity2 = None;
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entity1 = Some(entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)));
        entity2 = Some(entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)));
    });

    world.run::<(EntitiesMut, AllStorages), _>(|(mut entities, mut all_storages)| {
        assert!(entities.delete(&mut all_storages, entity1.unwrap()));
        assert!(!entities.delete(&mut all_storages, entity1.unwrap()));
    });

    world.run::<(&usize, &u32), _>(|(usizes, u32s)| {
        assert_eq!((&usizes).get(entity1.unwrap()), None);
        assert_eq!((&u32s).get(entity1.unwrap()), None);
        assert_eq!(usizes.get(entity2.unwrap()), Some(&2));
        assert_eq!(u32s.get(entity2.unwrap()), Some(&3));
        let mut iter = (&usizes, &u32s).iter();
        assert_eq!(iter.next(), Some((&2, &3)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn delete_tight_loose() {
    let world = World::new::<(usize, u64, u32)>();

    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();

    let mut entity1 = None;
    let mut entity2 = None;
    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _>(
        |(mut entities, mut usizes, mut u64s, mut u32s)| {
            entity1 = Some(entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2)));
            entity2 = Some(entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (3, 4, 5)));
        },
    );

    world.run::<(EntitiesMut, AllStorages), _>(|(mut entities, mut all_storages)| {
        assert!(entities.delete(&mut all_storages, entity1.unwrap()));
        assert!(!entities.delete(&mut all_storages, entity1.unwrap()));
    });

    world.run::<(&usize, &u64, &u32), _>(|(usizes, u64s, u32s)| {
        assert_eq!((&usizes).get(entity1.unwrap()), None);
        assert_eq!((&u64s).get(entity1.unwrap()), None);
        assert_eq!((&u32s).get(entity1.unwrap()), None);
        assert_eq!(usizes.get(entity2.unwrap()), Some(&3));
        assert_eq!(u64s.get(entity2.unwrap()), Some(&4));
        assert_eq!(u32s.get(entity2.unwrap()), Some(&5));
        let mut tight_iter = (&usizes, &u64s).iter();
        assert_eq!(tight_iter.next(), Some((&3, &4)));
        assert_eq!(tight_iter.next(), None);
        let mut loose_iter = (&usizes, &u64s, &u32s).iter();
        assert_eq!(loose_iter.next(), Some((&3, &4, &5)));
        assert_eq!(loose_iter.next(), None);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn thread_pool() {
    let world = World::new::<(usize, u32)>();
    world.run::<(ThreadPool,), _>(|(thread_pool,)| {
        use rayon::prelude::*;

        let vec = vec![0, 1, 2, 3];
        thread_pool.install(|| {
            assert_eq!(vec.into_par_iter().sum::<i32>(), 6);
        });
    })
}

#[test]
fn system() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a mut usize, &'a u32);
        fn run(&self, (usizes, u32s): <Self::Data as SystemData>::View) {
            for (x, y) in (usizes, u32s).iter() {
                *x += *y as usize;
            }
        }
    }

    let world = World::new::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    });

    world.add_workload("sys1", System1);
    world.run_default();
    world.run::<(&usize,), _>(|(usizes,)| {
        let mut iter = usizes.iter();
        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.next(), Some(&5));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn systems() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a mut usize, &'a u32);
        fn run(&self, (usizes, u32s): <Self::Data as SystemData>::View) {
            for (x, y) in (usizes, u32s).iter() {
                *x += *y as usize;
            }
        }
    }
    struct System2;
    impl<'a> System<'a> for System2 {
        type Data = (&'a mut usize,);
        fn run(&self, (usizes,): <Self::Data as SystemData>::View) {
            for x in (usizes,).iter() {
                *x += 1;
            }
        }
    }

    let world = World::new::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    });

    world.add_workload("sys1", (System1, System2));
    world.run_default();
    world.run::<(&usize,), _>(|(usizes,)| {
        let mut iter = usizes.iter();
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.next(), Some(&6));
        assert_eq!(iter.next(), None);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn simple_parallel_sum() {
    use rayon::prelude::*;

    let world = World::new::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (1usize, 2u32));
        entities.add_entity((&mut usizes, &mut u32s), (3usize, 4u32));
    });

    world.run::<(&mut usize, ThreadPool), _>(|(usizes, thread_pool)| {
        thread_pool.install(|| {
            let sum: usize = (&usizes,).par_iter().cloned().sum();
            assert_eq!(sum, 4);
        });
    });
}

#[cfg(feature = "parallel")]
#[test]
fn packed_parallel_iterator() {
    use iterators::ParIter2;
    use rayon::prelude::*;

    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    });

    world.run::<(&mut usize, &u32, ThreadPool), _>(|(mut usizes, u32s, thread_pool)| {
        let counter = std::sync::atomic::AtomicUsize::new(0);
        thread_pool.install(|| {
            if let ParIter2::Tight(iter) = (&mut usizes, &u32s).par_iter() {
                iter.for_each(|(x, y)| {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    *x += *y as usize;
                });
            } else {
                panic!()
            }
        });
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
        let mut iter = usizes.iter();
        assert_eq!(iter.next(), Some(&mut 1));
        assert_eq!(iter.next(), Some(&mut 5));
        assert_eq!(iter.next(), None);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_iterator() {
    use rayon::prelude::*;

    let world = World::new::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
        entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
    });

    world.run::<(&mut usize, &u32, ThreadPool), _>(|(mut usizes, u32s, thread_pool)| {
        let counter = std::sync::atomic::AtomicUsize::new(0);
        thread_pool.install(|| {
            (&mut usizes, &u32s).par_iter().for_each(|(x, y)| {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                *x += *y as usize;
            });
        });
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 2);
        let mut iter = usizes.iter();
        assert_eq!(iter.next(), Some(&mut 1));
        assert_eq!(iter.next(), Some(&mut 5));
        assert_eq!(iter.next(), None);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn two_workloads() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a usize,);
        fn run(&self, _: <Self::Data as SystemData>::View) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    let world = World::new::<(usize, u32)>();
    world.add_workload("default", (System1,));

    rayon::scope(|s| {
        s.spawn(|_| world.run_default());
        s.spawn(|_| world.run_default());
    });
}

#[cfg(feature = "parallel")]
#[test]
#[should_panic(
    expected = "Result::unwrap()` on an `Err` value: Cannot mutably borrow while already borrowed."
)]
fn two_bad_workloads() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a mut usize,);
        fn run(&self, _: <Self::Data as SystemData>::View) {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }

    let world = World::new::<(usize, u32)>();
    world.add_workload("default", (System1,));

    rayon::scope(|s| {
        s.spawn(|_| world.run_default());
        s.spawn(|_| world.run_default());
    });
}

#[test]
#[should_panic(expected = "Entity has to be alive to add component to it.")]
fn add_component_with_old_key() {
    let world = World::new::<(usize, u32)>();

    let mut entity = None;
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entity = Some(entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)));
    });

    world.run::<(EntitiesMut, AllStorages), _>(|(mut entities, mut all_storages)| {
        entities.delete(&mut all_storages, entity.unwrap());
    });

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(entities, mut usizes, mut u32s)| {
        entities.add_component((&mut usizes, &mut u32s), (1, 2), entity.unwrap());
    });
}

#[test]
fn remove_component_with_old_key() {
    let world = World::new::<(usize, u32)>();

    let mut entity = None;
    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entity = Some(entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)));
    });

    world.run::<(EntitiesMut, AllStorages), _>(|(mut entities, mut all_storages)| {
        entities.delete(&mut all_storages, entity.unwrap());
    });

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        let (old_usize, old_u32) =
            Remove::<(usize, u32)>::remove((&mut usizes, &mut u32s), entity.unwrap());
        assert!(old_usize.is_none() && old_u32.is_none());
    });
}

#[test]
fn derive() {
    let t = trybuild::TestCases::new();
    t.pass("tests/derive/good.rs");
    t.pass("tests/derive/return_nothing.rs");
    t.compile_fail("tests/derive/generic_lifetime.rs");
    t.compile_fail("tests/derive/generic_type.rs");
    t.compile_fail("tests/derive/not_entities.rs");
    t.compile_fail("tests/derive/not_run.rs");
    t.compile_fail("tests/derive/return_something.rs");
    t.compile_fail("tests/derive/where.rs");
    t.compile_fail("tests/derive/wrong_type.rs");
}

#[test]
fn pack() {
    use std::any::TypeId;

    let world = World::new::<(usize, u64, u32, u16)>();

    world.tight_pack::<(usize, u64)>();

    match world.try_tight_pack::<(usize, u64)>() {
        Ok(_) => panic!(),
        Err(err) => assert!(
            err == error::Pack::AlreadyTightPack(TypeId::of::<usize>())
                || err == error::Pack::AlreadyTightPack(TypeId::of::<u64>())
        ),
    }

    match world.try_tight_pack::<(usize, u32)>() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(err, error::Pack::AlreadyTightPack(TypeId::of::<usize>())),
    }

    match world.try_tight_pack::<(u64, u32)>() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(err, error::Pack::AlreadyTightPack(TypeId::of::<u64>())),
    }

    match world.try_loose_pack::<(usize, u64), (u32,)>() {
        Ok(_) => panic!(),
        Err(err) => assert!(
            err == error::Pack::AlreadyTightPack(TypeId::of::<usize>())
                || err == error::Pack::AlreadyTightPack(TypeId::of::<u64>())
        ),
    }

    match world.try_loose_pack::<(usize,), (u32,)>() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(err, error::Pack::AlreadyTightPack(TypeId::of::<usize>())),
    }

    match world.try_loose_pack::<(u64,), (u32,)>() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(err, error::Pack::AlreadyTightPack(TypeId::of::<u64>())),
    }

    world.loose_pack::<(u32,), (usize, u64)>();

    match world.try_tight_pack::<(u32, u16)>() {
        Ok(_) => panic!(),
        Err(err) => assert!(err == error::Pack::AlreadyLoosePack(TypeId::of::<u32>())),
    }

    match world.try_loose_pack::<(u32,), (u16,)>() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(err, error::Pack::AlreadyLoosePack(TypeId::of::<u32>())),
    }
}

#[test]
fn simple_sort() {
    let world = World::new::<(usize,)>();

    world.run::<(EntitiesMut, &mut usize), _>(|(mut entities, mut usizes)| {
        entities.add_entity(&mut usizes, 5);
        entities.add_entity(&mut usizes, 2);
        entities.add_entity(&mut usizes, 4);
        entities.add_entity(&mut usizes, 3);
        entities.add_entity(&mut usizes, 1);

        usizes.as_sortable().sort_unstable(Ord::cmp);

        let mut prev = 0;
        for &mut x in usizes.iter() {
            assert!(prev <= x);
            prev = x;
        }
    });
}

#[test]
fn tight_sort() {
    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (10usize, 3u32));
        entities.add_entity((&mut usizes, &mut u32s), (5usize, 9u32));
        entities.add_entity((&mut usizes, &mut u32s), (1usize, 5u32));
        entities.add_entity((&mut usizes, &mut u32s), (3usize, 54u32));

        (&mut usizes, &mut u32s)
            .as_sortable()
            .sort_unstable(|(&x1, &y1), (&x2, &y2)| (x1 + y1 as usize).cmp(&(x2 + y2 as usize)));

        let mut prev = 0;
        for (&mut x, &mut y) in (&mut usizes, &mut u32s).iter() {
            assert!(prev <= x + y as usize);
            prev = x + y as usize;
        }
    });
}

#[test]
fn loose_sort() {
    let world = World::new::<(usize, u32)>();
    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _>(|(mut entities, mut usizes, mut u32s)| {
        entities.add_entity((&mut usizes, &mut u32s), (10usize, 3u32));
        entities.add_entity((&mut usizes, &mut u32s), (5usize, 9u32));
        entities.add_entity((&mut usizes, &mut u32s), (1usize, 5u32));
        entities.add_entity((&mut usizes, &mut u32s), (3usize, 54u32));

        (&mut usizes, &mut u32s)
            .as_sortable()
            .sort_unstable(|(&x1, &y1), (&x2, &y2)| (x1 + y1 as usize).cmp(&(x2 + y2 as usize)));

        let mut prev = 0;
        for (&mut x, &mut y) in (&mut usizes, &mut u32s).iter() {
            assert!(prev <= x + y as usize);
            prev = x + y as usize;
        }
    });
}

#[test]
fn tight_loose_sort() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();

    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _>(
        |(mut entities, mut usizes, mut u64s, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u64s), (3, 4));
            entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (6, 7, 8));
            entities.add_entity((&mut usizes,), (5,));
            entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));

            (&mut usizes, &mut u64s)
                .as_sortable()
                .sort_unstable(|(&x1, &y1), (&x2, &y2)| {
                    (x1 + y1 as usize).cmp(&(x2 + y2 as usize))
                });
        },
    );

    world.run::<(&usize, &u64, &u32), _>(|(usizes, u64s, u32s)| {
        if let iterators::Iter3::Loose(mut iter) = (&usizes, &u64s, &u32s).iter() {
            assert_eq!(iter.next(), Some((&6, &7, &8)));
            assert_eq!(iter.next(), Some((&0, &1, &2)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not loose");
        }
        if let iterators::Iter2::Tight(mut iter) = (&usizes, &u64s).iter() {
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), Some((&3, &4)));
            assert_eq!(iter.next(), Some((&6, &7)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not tight");
        }
        if let iterators::Iter2::NonPacked(mut iter) = (&usizes, &u32s).iter() {
            assert_eq!(iter.next(), Some((&6, &8)));
            assert_eq!(iter.next(), Some((&0, &2)));
            assert_eq!(iter.next(), None);
        }
    });
}

#[test]
#[should_panic(
    expected = "The storage you want to sort is packed, you may be able to sort the whole pack by passing all storages packed with it to the function. Some packs can't be sorted."
)]
fn tight_sort_missing_storage() {
    let world = World::new::<(usize, u64)>();
    world.tight_pack::<(usize, u64)>();

    world.run::<(&mut usize,), _>(|(mut usizes,)| {
        &mut usizes.as_sortable().sort_unstable(Ord::cmp);
    });
}

#[test]
#[should_panic(
    expected = "The storage you want to sort is packed, you may be able to sort the whole pack by passing all storages packed with it to the function. Some packs can't be sorted."
)]
fn loose_sort_missing_storage() {
    let world = World::new::<(usize, u64)>();
    world.loose_pack::<(usize,), (u64,)>();

    world.run::<(&mut usize,), _>(|(mut usizes,)| {
        &mut usizes.as_sortable().sort_unstable(Ord::cmp);
    });
}

#[test]
#[should_panic(
    expected = "You provided too many storages non packed together. Only single storage and storages packed together can be sorted."
)]
fn tight_sort_too_many_storages() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();

    world.run::<(&mut usize, &mut u64, &mut u32), _>(|(mut usizes, mut u64s, mut u32s)| {
        (&mut usizes, &mut u64s, &mut u32s)
            .as_sortable()
            .sort_unstable(|(&x1, &y1, &z1), (&x2, &y2, &z2)| {
                (x1 + y1 as usize + z1 as usize).cmp(&(x2 + y2 as usize + z2 as usize))
            });
    });
}
