use rayon::prelude::*;
use shipyard::*;

#[test]
fn filter() {
    let world = World::new_with_custom_lock::<parking_lot::RawRwLock>();
    let (mut entities, mut u32s) = world.borrow::<(EntitiesViewMut, ViewMut<u32>)>().unwrap();

    u32s.track_all();
    entities.add_entity(&mut u32s, 0);
    entities.add_entity(&mut u32s, 1);
    entities.add_entity(&mut u32s, 2);
    entities.add_entity(&mut u32s, 3);
    entities.add_entity(&mut u32s, 4);
    entities.add_entity(&mut u32s, 5);
    u32s.clear_all_inserted();

    let im_vec;
    let m_vec;
    let mod_vec;

    let iter = u32s.par_iter();
    assert_eq!(iter.opt_len(), Some(6));
    im_vec = iter.filter(|&&x| x % 2 == 0).collect::<Vec<_>>();
    assert_eq!(im_vec, vec![&0, &2, &4]);
    drop(im_vec);

    m_vec = (&mut u32s)
        .par_iter()
        .filter(|x| **x % 2 != 0)
        .map(|mut x| {
            *x += 1;
            x
        })
        .map(|x| *x)
        .collect::<Vec<_>>();
    assert_eq!(m_vec, vec![2, 4, 6]);
    mod_vec = u32s.modified().par_iter().collect::<Vec<_>>();

    assert_eq!(mod_vec, vec![&2, &4, &6]);
}
