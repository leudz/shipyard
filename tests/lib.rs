mod iteration;
mod sort;
mod static_view;
mod workload;

#[cfg(feature = "serialization")]
mod serialization;
use shipyard::internal::iterators;
use shipyard::prelude::*;

#[test]
fn add_entity() {
    let world = World::default();
    world.run::<EntitiesMut, _, _>(|mut entities| {
        entities.add_entity((), ());
    });
    world.register::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
        },
    );
}

#[test]
fn tight_add_entity() {
    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
        },
    );
}

#[test]
fn loose_add_entity() {
    let world = World::new::<(usize, u32)>();
    world.loose_pack::<(usize,), (u32,)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0, 1));
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&0, &1));
        },
    );
}

#[test]
fn tight_loose_add_entity() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();

    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u64s, mut u32s)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
            assert_eq!((&usizes, &u64s, &u32s).get(entity1).unwrap(), (&0, &1, &2));
        },
    );
}

#[test]
fn add_component() {
    let world = World::new::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            let entity1 = entities.add_entity((), ());
            entities.add_component((&mut usizes, &mut u32s), (0, 1), entity1);
            entities.add_component((&mut u32s, &mut usizes), (3, 2), entity1);
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
        },
    );
}

#[test]
fn tight_add_component() {
    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            let entity1 = entities.add_entity((), ());
            entities.add_component((&mut usizes, &mut u32s), (0, 1), entity1);
            entities.add_component((&mut usizes, &mut u32s), (3usize,), entity1);
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&3, &1));
            let mut iter = (&usizes, &u32s).iter();
            assert_eq!(iter.next(), Some((&3, &1)));
            assert_eq!(iter.next(), None);
        },
    );
}

#[test]
fn loose_add_component() {
    let world = World::new::<(usize, u32)>();
    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            let entity1 = entities.add_entity((), ());
            entities.add_component((&mut usizes, &mut u32s), (0, 1), entity1);
            entities.add_component((&mut u32s, &mut usizes), (3, 2), entity1);
            assert_eq!((&usizes, &u32s).get(entity1).unwrap(), (&2, &3));
            let mut iter = (&usizes, &u32s).iter();
            assert_eq!(iter.next(), Some((&2, &3)));
            assert_eq!(iter.next(), None);
        },
    );
}

#[test]
fn tight_loose_add_component() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();
    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _, _>(
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
    world.run::<(EntitiesMut, &mut usize, &mut u32, &mut String), _, _>(
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
    world.run::<(EntitiesMut, &mut usize, &mut u32, &mut String), _, _>(
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

    world.run::<(EntitiesMut, &mut usize, &mut u32, &mut String), _, _>(
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
    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32, &mut String), _, _>(
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
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));

            // possible to borrow twice as immutable
            let mut iter1 = (&usizes).iter();
            let _iter2 = (&usizes).iter();
            assert_eq!(iter1.next(), Some(&0));

            // impossible to borrow twice as mutable
            // if switched, the next two lines should trigger an shipyard::error
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
            // if switched, the next two lines should trigger an shipyard::error
            let _iter = (&mut usizes, &u32s).iter();
            let mut iter = (&mut usizes, &u32s).iter();
            assert_eq!(iter.next(), Some((&mut 0, &1)));
            assert_eq!(iter.next(), Some((&mut 2, &3)));
            assert_eq!(iter.next(), None);
        },
    );
}

#[test]
fn iterators() {
    let world = World::new::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
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
        },
    );
}

#[test]
fn not_iterators() {
    let world = World::new::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
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
        },
    );
}

#[test]
fn pack_missing_storage() {
    match std::panic::catch_unwind(|| {
        let world = World::new::<(usize, u32)>();
        world.tight_pack::<(usize, u32)>();

        world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(mut entities, mut usizes, mut u32s)| {
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

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes,), (2usize,));
            // test for consuming version
            entities.add_entity((usizes, u32s), (3usize, 4u32));
        },
    );

    world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
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

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes,), (2usize,));
            // test for consuming version
            entities.add_entity((usizes, u32s), (3usize, 4u32));
        },
    );

    world.tight_pack::<(usize, u32)>();

    world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
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

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
            entities.add_entity((&mut usizes, &mut u32s), (4usize, 5u32));
            entities.add_entity((&mut usizes, &mut u32s), (6usize, 7u32));
            entities.add_entity((&mut usizes, &mut u32s), (8usize, 9u32));
            entities.add_entity((&mut usizes,), (10usize,));
        },
    );

    world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
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
            for _ in 0..=3 {
                iter.next();
            }
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

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes,), (2usize,));
            // test for consuming version
            entities.add_entity((usizes, u32s), (3usize, 4u32));
        },
    );

    world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
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

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes,), (2usize,));
            // test for consuming version
            entities.add_entity((usizes, u32s), (3usize, 4u32));
        },
    );

    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
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

    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u64s, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2));
            entities.add_entity((&mut usizes, &mut u64s), (3, 4));
            entities.add_entity((&mut usizes,), (5,));
            // test for consuming version
            entities.add_entity((usizes, u64s, u32s), (6, 7, 8));
        },
    );

    world.run::<(&usize, &u64, &u32), _, _>(|(usizes, u64s, u32s)| {
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

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            let entity1 = entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            let entity2 = entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
            let component = Remove::<(usize,)>::remove((&mut usizes,), entity1);
            assert_eq!(component, (Some(0usize),));
            assert_eq!((&mut usizes).get(entity1), None);
            assert_eq!((&mut u32s).get(entity1), Some(&mut 1));
            assert_eq!(usizes.get(entity2), Some(&2));
            assert_eq!(u32s.get(entity2), Some(&3));
        },
    );
}

#[test]
fn remove_tight() {
    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
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
        },
    );
}

#[test]
fn remove_loose() {
    let world = World::new::<(usize, u32)>();
    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
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
        },
    );
}

#[test]
fn remove_tight_loose() {
    let world = World::new::<(usize, u64, u32)>();
    world.tight_pack::<(usize, u64)>();
    world.loose_pack::<(u32,), (usize, u64)>();

    world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _, _>(
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

    let (entity1, entity2) = world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            (
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)),
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)),
            )
        },
    );

    world.run::<AllStorages, _, _>(|mut all_storages| {
        assert!(all_storages.delete(entity1));
        assert!(!all_storages.delete(entity1));
    });

    world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
        assert_eq!((&usizes).get(entity1), None);
        assert_eq!((&u32s).get(entity1), None);
        assert_eq!(usizes.get(entity2), Some(&2));
        assert_eq!(u32s.get(entity2), Some(&3));
    });
}

#[test]
fn delete_tight() {
    let world = World::new::<(usize, u32)>();

    world.tight_pack::<(usize, u32)>();

    let (entity1, entity2) = world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            (
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)),
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)),
            )
        },
    );

    world.run::<AllStorages, _, _>(|mut all_storages| {
        assert!(all_storages.delete(entity1));
        assert!(!all_storages.delete(entity1));
    });

    world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
        assert_eq!((&usizes).get(entity1), None);
        assert_eq!((&u32s).get(entity1), None);
        assert_eq!(usizes.get(entity2), Some(&2));
        assert_eq!(u32s.get(entity2), Some(&3));
        let mut iter = (&usizes, &u32s).iter();
        assert_eq!(iter.next(), Some((&2, &3)));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn delete_loose() {
    let world = World::new::<(usize, u32)>();

    world.loose_pack::<(usize,), (u32,)>();

    let (entity1, entity2) = world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            (
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)),
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)),
            )
        },
    );

    world.run::<AllStorages, _, _>(|mut all_storages| {
        assert!(all_storages.delete(entity1));
        assert!(!all_storages.delete(entity1));
    });

    world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
        assert_eq!((&usizes).get(entity1), None);
        assert_eq!((&u32s).get(entity1), None);
        assert_eq!(usizes.get(entity2), Some(&2));
        assert_eq!(u32s.get(entity2), Some(&3));
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

    let (entity1, entity2) = world.run::<(EntitiesMut, &mut usize, &mut u64, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u64s, mut u32s)| {
            (
                entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (0, 1, 2)),
                entities.add_entity((&mut usizes, &mut u64s, &mut u32s), (3, 4, 5)),
            )
        },
    );

    world.run::<AllStorages, _, _>(|mut all_storages| {
        assert!(all_storages.delete(entity1));
        assert!(!all_storages.delete(entity1));
    });

    world.run::<(&usize, &u64, &u32), _, _>(|(usizes, u64s, u32s)| {
        assert_eq!((&usizes).get(entity1), None);
        assert_eq!((&u64s).get(entity1), None);
        assert_eq!((&u32s).get(entity1), None);
        assert_eq!(usizes.get(entity2), Some(&3));
        assert_eq!(u64s.get(entity2), Some(&4));
        assert_eq!(u32s.get(entity2), Some(&5));
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
    world.run::<(ThreadPool,), _, _>(|(thread_pool,)| {
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
        fn run((usizes, u32s): <Self::Data as SystemData>::View) {
            (usizes, u32s).iter().for_each(|(x, y)| {
                *x += *y as usize;
            });
        }
    }

    let world = World::new::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        },
    );

    world.add_workload("sys1", System1);
    world.run_default();
    world.run::<(&usize,), _, _>(|(usizes,)| {
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
        fn run((usizes, u32s): <Self::Data as SystemData>::View) {
            (usizes, u32s).iter().for_each(|(x, y)| {
                *x += *y as usize;
            });
        }
    }
    struct System2;
    impl<'a> System<'a> for System2 {
        type Data = (&'a mut usize,);
        fn run((usizes,): <Self::Data as SystemData>::View) {
            (usizes,).iter().for_each(|x| {
                *x += 1;
            });
        }
    }

    let world = World::new::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        },
    );

    world.add_workload("sys1", (System1, System2));
    world.run_default();
    world.run::<(&usize,), _, _>(|(usizes,)| {
        let mut iter = usizes.iter();
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.next(), Some(&6));
        assert_eq!(iter.next(), None);
    });
}
/*
#[cfg(feature = "parallel")]
#[test]
fn simple_parallel_sum() {
    use rayon::prelude::*;

    let world = World::new::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (1usize, 2u32));
            entities.add_entity((&mut usizes, &mut u32s), (3usize, 4u32));
        },
    );

    world.run::<(&mut usize, ThreadPool), _, _>(|(usizes, thread_pool)| {
        thread_pool.install(|| {
            let sum: usize = (&usizes,).par_iter().cloned().sum();
            assert_eq!(sum, 4);
        });
    });
}

#[cfg(feature = "parallel")]
#[test]
fn tight_parallel_iterator() {
    use iterators::ParIter2;
    use rayon::prelude::*;

    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        },
    );

    world.run::<(&mut usize, &u32, ThreadPool), _, _>(|(mut usizes, u32s, thread_pool)| {
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

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        },
    );

    world.run::<(&mut usize, &u32, ThreadPool), _, _>(|(mut usizes, u32s, thread_pool)| {
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
fn loose_parallel_iterator() {
    use iterators::ParIter2;
    use rayon::prelude::*;

    let world = World::new::<(usize, u32)>();
    world.loose_pack::<(usize,), (u32,)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
        },
    );

    world.run::<(&mut usize, &u32, ThreadPool), _, _>(|(mut usizes, u32s, thread_pool)| {
        let counter = std::sync::atomic::AtomicUsize::new(0);
        thread_pool.install(|| {
            if let ParIter2::Loose(iter) = (&mut usizes, &u32s).par_iter() {
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
*/
#[cfg(feature = "parallel")]
#[test]
fn two_workloads() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a usize,);
        fn run(_: <Self::Data as SystemData>::View) {
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
    expected = "Result::unwrap()` on an `Err` value: Cannot mutably borrow usize storage while it's already borrowed."
)]
fn two_bad_workloads() {
    struct System1;
    impl<'a> System<'a> for System1 {
        type Data = (&'a mut usize,);
        fn run(_: <Self::Data as SystemData>::View) {
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

    let entity = world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32))
        },
    );

    world.run::<AllStorages, _, _>(|mut all_storages| {
        all_storages.delete(entity);
    });

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(|(entities, mut usizes, mut u32s)| {
        entities.add_component((&mut usizes, &mut u32s), (1, 2), entity);
    });
}

#[test]
fn remove_component_with_old_key() {
    let world = World::new::<(usize, u32)>();

    let entity = world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32))
        },
    );

    world.run::<AllStorages, _, _>(|mut all_storages| {
        all_storages.delete(entity);
    });

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32));
            let (old_usize, old_u32) =
                Remove::<(usize, u32)>::remove((&mut usizes, &mut u32s), entity);
            assert!(old_usize.is_none() && old_u32.is_none());
        },
    );
}

#[test]
fn compile_err() {
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
    t.compile_fail("tests/compile_err/taken_from_run.rs");
}

#[test]
fn pack() {
    use std::any::TypeId;

    let world = World::new::<(usize, u64, u32, u16)>();

    world.tight_pack::<(usize, u64)>();

    match world.try_tight_pack::<(usize, u64)>() {
        Ok(_) => panic!(),
        Err(err) => assert!(
            err == shipyard::error::Pack::AlreadyTightPack(TypeId::of::<usize>())
                || err == shipyard::error::Pack::AlreadyTightPack(TypeId::of::<u64>())
        ),
    }

    match world.try_tight_pack::<(usize, u32)>() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            err,
            shipyard::error::Pack::AlreadyTightPack(TypeId::of::<usize>())
        ),
    }

    match world.try_tight_pack::<(u64, u32)>() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            err,
            shipyard::error::Pack::AlreadyTightPack(TypeId::of::<u64>())
        ),
    }

    match world.try_loose_pack::<(usize, u64), (u32,)>() {
        Ok(_) => panic!(),
        Err(err) => assert!(
            err == shipyard::error::Pack::AlreadyTightPack(TypeId::of::<usize>())
                || err == shipyard::error::Pack::AlreadyTightPack(TypeId::of::<u64>())
        ),
    }

    match world.try_loose_pack::<(usize,), (u32,)>() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            err,
            shipyard::error::Pack::AlreadyTightPack(TypeId::of::<usize>())
        ),
    }

    match world.try_loose_pack::<(u64,), (u32,)>() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            err,
            shipyard::error::Pack::AlreadyTightPack(TypeId::of::<u64>())
        ),
    }

    world.loose_pack::<(u32,), (usize, u64)>();

    match world.try_tight_pack::<(u32, u16)>() {
        Ok(_) => panic!(),
        Err(err) => assert!(err == shipyard::error::Pack::AlreadyLoosePack(TypeId::of::<u32>())),
    }

    match world.try_loose_pack::<(u32,), (u16,)>() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            err,
            shipyard::error::Pack::AlreadyLoosePack(TypeId::of::<u32>())
        ),
    }
}

#[test]
fn simple_filter() {
    let world = World::new::<(usize,)>();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        entities.add_entity(&mut usizes, 5);
        entities.add_entity(&mut usizes, 2);
        entities.add_entity(&mut usizes, 4);
        entities.add_entity(&mut usizes, 3);
        entities.add_entity(&mut usizes, 1);

        let mut iter = usizes.iter().filter(|&&mut x| x % 2 == 0);

        assert_eq!(iter.next(), Some(&mut 2));
        assert_eq!(iter.next(), Some(&mut 4));
        assert_eq!(iter.next(), None);
    });
}

#[test]
fn tight_filter() {
    let world = World::new::<(usize, u32)>();
    world.tight_pack::<(usize, u32)>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0, 1));
            entities.add_entity((&mut usizes,), (2,));
            entities.add_entity((&mut usizes, &mut u32s), (3, 4));

            let mut iter = (&usizes, &u32s).iter().filter(|&(x, _)| x % 2 == 0);

            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), None);
        },
    );
}

#[test]
fn update_pack() {
    let world = World::new::<(usize,)>();
    world.update_pack::<usize>();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        entities.add_entity(&mut usizes, 0);
        entities.add_entity(&mut usizes, 1);
        entities.add_entity(&mut usizes, 2);
        entities.add_entity(&mut usizes, 3);

        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), Some(&0));
        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.next(), None);

        let mut iter = usizes.modified().iter();
        assert_eq!(iter.next(), None);

        usizes.clear_inserted();

        let mut iter = (&mut usizes).iter().filter(|&&mut x| x % 2 == 0);
        assert_eq!(iter.next(), Some(&mut 0));
        assert_eq!(iter.next(), Some(&mut 2));
        assert_eq!(iter.next(), None);

        let mut iter = usizes.modified_mut().iter();
        assert_eq!(iter.next(), Some(&mut 0));
        assert_eq!(iter.next(), Some(&mut 2));
        assert_eq!(iter.next(), None);

        usizes.clear_modified();

        let mut iter = usizes.modified().iter();
        assert_eq!(iter.next(), None);
    });
}
/*
#[cfg(feature = "parallel")]
#[test]
fn par_update_pack() {
    use rayon::prelude::*;

    let world = World::new::<(usize,)>();
    world.update_pack::<usize>();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        entities.add_entity(&mut usizes, 0);
        entities.add_entity(&mut usizes, 1);
        entities.add_entity(&mut usizes, 2);
        entities.add_entity(&mut usizes, 3);

        usizes.clear_inserted();

        (&usizes).par_iter().sum::<usize>();

        assert_eq!(usizes.modified().len(), 0);

        (&mut usizes).par_iter().for_each(|i| {
            *i += 1;
        });

        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), None);

        let mut iter = usizes.modified_mut().iter();
        assert_eq!(iter.next(), Some(&mut 1));
        assert_eq!(iter.next(), Some(&mut 2));
        assert_eq!(iter.next(), Some(&mut 3));
        assert_eq!(iter.next(), Some(&mut 4));
        assert_eq!(iter.next(), None);
    });
}
*/
#[test]
fn simple_with_id() {
    let world = World::new::<(usize, u32)>();
    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            let entity1 = entities.add_entity((&mut usizes,), (0usize,));
            entities.add_component((&mut u32s,), (1u32,), entity1);
            let entity2 = entities.add_entity((&mut usizes,), (2usize,));

            let mut iter = (&usizes).iter().with_id();
            let item = iter.next().unwrap();
            assert!(item.0 == entity1 && item.1 == &0);
            let item = iter.next().unwrap();
            assert!(item.0 == entity2 && item.1 == &2);
            assert!(iter.next().is_none());

            let mut iter = (&u32s).iter().with_id();
            let item = iter.next().unwrap();
            assert!(item.0 == entity1 && item.1 == &1);
            assert!(iter.next().is_none());
        },
    );
}

#[test]
fn multiple_update_pack() {
    use iterators::Iter2;

    let world = World::new::<(usize, u32)>();
    world.update_pack::<u32>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity(&mut usizes, 2usize);
            entities.add_entity((&mut usizes, &mut u32s), (4usize, 5u32));
            entities.add_entity(&mut u32s, 7u32);
            entities.add_entity((&mut usizes, &mut u32s), (8usize, 9u32));
            entities.add_entity((&mut usizes,), (10usize,));

            u32s.clear_inserted();
        },
    );

    world.run::<(&mut usize, &mut u32), _, _>(|(mut usizes, mut u32s)| {
        if let Iter2::Update(mut iter) = (&usizes, &u32s).iter() {
            assert_eq!(iter.next(), Some((&0, &1)));
            assert_eq!(iter.next(), Some((&4, &5)));
            assert_eq!(iter.next(), Some((&8, &9)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }

        assert_eq!(u32s.modified().len(), 0);

        if let Iter2::Update(mut iter) = (&mut usizes, &u32s).iter() {
            assert_eq!(iter.next(), Some((&mut 0, &1)));
            assert_eq!(iter.next(), Some((&mut 4, &5)));
            assert_eq!(iter.next(), Some((&mut 8, &9)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }

        assert_eq!(u32s.modified().len(), 0);

        if let Iter2::Update(mut iter) = (&usizes, &mut u32s).iter() {
            assert_eq!(iter.next(), Some((&0, &mut 1)));
            assert_eq!(iter.next(), Some((&4, &mut 5)));
            assert_eq!(iter.next(), Some((&8, &mut 9)));
            assert_eq!(iter.next(), None);
        } else {
            panic!("not packed");
        }

        let mut modified = u32s.modified().iter();
        assert_eq!(modified.next(), Some(&1));
        assert_eq!(modified.next(), Some(&5));
        assert_eq!(modified.next(), Some(&9));
        assert_eq!(modified.next(), None);

        let mut iter = (&u32s).iter();
        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.next(), Some(&5));
        assert_eq!(iter.next(), Some(&9));
        assert_eq!(iter.next(), Some(&7));
        assert_eq!(iter.next(), None);
    });
}
/*
#[cfg(feature = "parallel")]
#[test]
fn par_multiple_update_pack() {
    use iterators::ParIter2;
    use rayon::prelude::*;

    let world = World::new::<(usize, u32)>();
    world.update_pack::<u32>();

    world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32));
            entities.add_entity(&mut usizes, 2usize);
            entities.add_entity((&mut usizes, &mut u32s), (4usize, 5u32));
            entities.add_entity(&mut u32s, 7u32);
            entities.add_entity((&mut usizes, &mut u32s), (8usize, 9u32));
            entities.add_entity((&mut usizes,), (10usize,));

            u32s.clear_inserted();
        },
    );

    world.run::<(&mut usize, &mut u32), _, _>(|(mut usizes, mut u32s)| {
        if let ParIter2::Update(iter) = (&usizes, &u32s).par_iter() {
            iter.for_each(|_| {});
        } else {
            panic!("not packed");
        }

        assert_eq!(u32s.modified().len(), 0);

        if let ParIter2::Update(iter) = (&mut usizes, &u32s).par_iter() {
            iter.for_each(|_| {});
        } else {
            panic!("not packed");
        }

        assert_eq!(u32s.modified().len(), 0);

        if let ParIter2::Update(iter) = (&usizes, &mut u32s).par_iter() {
            iter.for_each(|_| {});
        } else {
            panic!("not packed");
        }

        let mut modified: Vec<_> = u32s.modified().iter().collect();
        modified.sort_unstable();
        assert_eq!(modified, vec![&1, &5, &9]);

        let mut iter: Vec<_> = (&u32s).iter().collect();
        iter.sort_unstable();
        assert_eq!(iter, vec![&1, &5, &7, &9]);
    });
}

#[cfg(feature = "parallel")]
#[test]
fn par_update_filter() {
    use rayon::prelude::*;

    let world = World::new::<(usize,)>();
    world.update_pack::<usize>();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        entities.add_entity(&mut usizes, 0);
        entities.add_entity(&mut usizes, 1);
        entities.add_entity(&mut usizes, 2);
        entities.add_entity(&mut usizes, 3);

        usizes.clear_inserted();

        (&mut usizes)
            .par_iter()
            .filtered(|x| **x % 2 == 0)
            .for_each(|i| {
                *i += 1;
            });

        let mut iter = usizes.inserted().iter();
        assert_eq!(iter.next(), None);

        let mut modified: Vec<_> = usizes.modified().iter().collect();
        modified.sort_unstable();
        assert_eq!(modified, vec![&1, &3]);

        let mut iter: Vec<_> = (&usizes).iter().collect();
        iter.sort_unstable();
        assert_eq!(iter, vec![&1, &1, &3, &3]);
    });
}
*/
#[test]
fn filter_with_id() {
    let world = World::new::<(usize,)>();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        entities.add_entity(&mut usizes, 5);
        let entity1 = entities.add_entity(&mut usizes, 2);
        let entity2 = entities.add_entity(&mut usizes, 4);
        entities.add_entity(&mut usizes, 3);
        entities.add_entity(&mut usizes, 1);

        let mut iter = usizes.iter().filter(|&&mut x| x % 2 == 0).with_id();

        assert!(iter.next() == Some((entity1, &mut 2)));
        assert!(iter.next() == Some((entity2, &mut 4)));
        assert!(iter.next() == None);
    });
}
/*
#[cfg(feature = "parallel")]
#[test]
fn par_filter_with_id() {
    use rayon::iter::ParallelIterator;

    let world = World::new::<(usize,)>();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        entities.add_entity(&mut usizes, 5);
        let entity1 = entities.add_entity(&mut usizes, 2);
        let entity2 = entities.add_entity(&mut usizes, 4);
        entities.add_entity(&mut usizes, 3);
        entities.add_entity(&mut usizes, 1);

        let mut result: Vec<_> = usizes
            .par_iter()
            .filtered(|&&mut x| x % 2 == 0)
            .with_id()
            .collect();
        result.sort_by(|(_, x), (_, y)| x.cmp(y));

        assert!(result == vec![(entity1, &mut 2), (entity2, &mut 4)]);
    });
}
*/
#[test]
fn with_id_filter() {
    let world = World::new::<(usize,)>();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        entities.add_entity(&mut usizes, 5);
        let entity1 = entities.add_entity(&mut usizes, 2);
        let entity2 = entities.add_entity(&mut usizes, 4);
        entities.add_entity(&mut usizes, 3);
        entities.add_entity(&mut usizes, 1);

        let mut iter = usizes.iter().with_id().filter(|&(_, &mut x)| x % 2 == 0);

        assert!(iter.next() == Some((entity1, &mut 2)));
        assert!(iter.next() == Some((entity2, &mut 4)));
        assert!(iter.next() == None);
    });
}
/*
#[cfg(feature = "parallel")]
#[test]
fn par_with_id_filter() {
    use rayon::iter::ParallelIterator;

    let world = World::new::<(usize,)>();

    world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
        entities.add_entity(&mut usizes, 5);
        let entity1 = entities.add_entity(&mut usizes, 2);
        let entity2 = entities.add_entity(&mut usizes, 4);
        entities.add_entity(&mut usizes, 3);
        entities.add_entity(&mut usizes, 1);

        let mut result: Vec<_> = usizes
            .par_iter()
            .with_id()
            .filtered(|&(_, &mut x)| x % 2 == 0)
            .collect();
        result.sort_by(|(_, x), (_, y)| x.cmp(y));

        assert!(result == vec![(entity1, &mut 2), (entity2, &mut 4)]);
    });
}
*/
#[test]
fn unique_storage() {
    let world = World::default();
    world.add_unique(0usize);

    world.run::<Unique<&mut usize>, _, _>(|x| {
        *x += 1;
    });
    world.run::<Unique<&usize>, _, _>(|x| {
        assert_eq!(x, &1);
    });
}

#[test]
fn not_unique_storage() {
    match std::panic::catch_unwind(|| {
        let world = World::new::<(usize,)>();

        world.run::<Unique<&usize>, _, _>(|x| {
            assert_eq!(x, &1);
        });
    }) {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            format!("{}", err.downcast::<String>().unwrap()),
            "called `Result::unwrap()` on an `Err` value: usize's storage isn't unique.\n\
            You might have forgotten to declare it, replace world.register::<usize>() by world.register_unique(/* your_storage */).\n\
            If it isn't supposed to be a unique storage, replace Unique<&usize> by &usize."
        ),
    }

    match std::panic::catch_unwind(|| {
        let world = World::new::<(usize,)>();

        world.run::<Unique<&mut usize>, _, _>(|x| {
            assert_eq!(x, &1);
        });
    }) {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            format!("{}", err.downcast::<String>().unwrap()),
            "called `Result::unwrap()` on an `Err` value: usize's storage isn't unique.\n\
            You might have forgotten to declare it, replace world.register::<usize>() by world.register_unique(/* your_storage */).\n\
            If it isn't supposed to be a unique storage, replace Unique<&mut usize> by &mut usize."
        ),
    }
}

#[test]
fn key_equality() {
    let world = World::default();
    world.register::<(usize, u32)>();

    //create 3 entities
    let (e0, e1, e2) =
        world.run::<(EntitiesMut, &mut usize), _, _>(|(mut entities, mut usizes)| {
            (
                entities.add_entity(&mut usizes, 0),
                entities.add_entity(&mut usizes, 1),
                entities.add_entity(&mut usizes, 2),
            )
        });

    //add a component to e1
    world.run::<(EntitiesMut, &mut u32), _, _>(|(ref mut entities, ref mut u32s)| {
        entities.add_component(u32s, 42, e1);
    });

    //confirm that the entity keys have not changed for usizes storage
    world.run::<&usize, _, _>(|usizes| {
        //sanity check
        assert_eq!((&usizes).iter().with_id().count(), 3);

        let keys: Vec<EntityId> =
            (&usizes)
                .iter()
                .with_id()
                .map(|(entity, _)| entity)
                .fold(Vec::new(), |mut vec, x| {
                    vec.push(x);
                    vec
                });

        assert_eq!(keys, vec![e0, e1, e2]);
    });

    //confirm that the entity id for (usize) is the same as (usize, u32)
    //in other words that the entity itself did not somehow change from adding a component
    world.run::<(&usize, &u32), _, _>(|(usizes, u32s)| {
        //sanity check
        assert_eq!((&usizes, &u32s).iter().with_id().count(), 1);

        let (entity, (_, _)) = (&usizes, &u32s).iter().with_id().find(|_| true).unwrap();
        assert_eq!(entity, e1);
    });
}

#[test]
fn unique_storage_pack() {
    let world = World::new::<(u32,)>();
    world.add_unique(0usize);

    assert_eq!(
        world.try_tight_pack::<(u32, usize)>(),
        Err(shipyard::error::Pack::UniqueStorage(std::any::type_name::<
            usize,
        >()))
    );
    assert_eq!(
        world.try_loose_pack::<(u32,), (usize,)>(),
        Err(shipyard::error::Pack::UniqueStorage(std::any::type_name::<
            usize,
        >()))
    );
    assert_eq!(
        world.try_loose_pack::<(usize,), (u32,)>(),
        Err(shipyard::error::Pack::UniqueStorage(std::any::type_name::<
            usize,
        >()))
    );
    assert_eq!(
        world.try_update_pack::<usize>(),
        Err(shipyard::error::Pack::UniqueStorage(std::any::type_name::<
            usize,
        >()))
    );
}

#[test]
fn strip() {
    let world = World::new::<(usize, u32)>();

    let (entity1, entity2) = world.run::<(EntitiesMut, &mut usize, &mut u32), _, _>(
        |(mut entities, mut usizes, mut u32s)| {
            (
                entities.add_entity((&mut usizes, &mut u32s), (0usize, 1u32)),
                entities.add_entity((&mut usizes, &mut u32s), (2usize, 3u32)),
            )
        },
    );

    world.run::<AllStorages, _, _>(|mut all_storages| {
        all_storages.strip(entity1);
    });

    world.run::<(&mut usize, &mut u32), _, _>(|(mut usizes, mut u32s)| {
        assert_eq!((&mut usizes).get(entity1), None);
        assert_eq!((&mut u32s).get(entity1), None);
        assert_eq!(usizes.get(entity2), Some(&2));
        assert_eq!(u32s.get(entity2), Some(&3));
    });

    world.run::<AllStorages, _, _>(|mut all_storages| {
        assert!(all_storages.delete(entity1));
    });
}
