use rayon::prelude::*;
use shipyard::*;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
struct U64(u64);
impl Component for U64 {
    type Tracking = track::All;
}

#[test]
fn filter() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut u64s) = world.borrow::<(EntitiesViewMut, ViewMut<U64>)>().unwrap();

    entities.add_entity(&mut u64s, U64(0));
    entities.add_entity(&mut u64s, U64(1));
    entities.add_entity(&mut u64s, U64(2));
    entities.add_entity(&mut u64s, U64(3));
    entities.add_entity(&mut u64s, U64(4));
    entities.add_entity(&mut u64s, U64(5));
    u64s.clear_all_inserted();

    let mut u64s = world.borrow::<ViewMut<U64>>().unwrap();

    let im_vec;
    let m_vec;
    let mod_vec;

    let iter = u64s.par_iter();
    assert_eq!(iter.opt_len(), Some(6));
    im_vec = iter.filter(|&&x| x.0 % 2 == 0).collect::<Vec<_>>();
    assert_eq!(im_vec, vec![&U64(0), &U64(2), &U64(4)]);
    drop(im_vec);

    m_vec = (&mut u64s)
        .par_iter()
        .filter(|x| x.0 % 2 != 0)
        .map(|mut x| {
            x.0 += 1;
            x
        })
        .map(|x| *x)
        .collect::<Vec<_>>();
    assert_eq!(m_vec, vec![U64(2), U64(4), U64(6)]);
    mod_vec = u64s.modified().par_iter().collect::<Vec<_>>();

    assert_eq!(mod_vec, vec![&U64(2), &U64(4), &U64(6)]);
}
