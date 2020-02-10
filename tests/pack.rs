use shipyard::prelude::*;

#[test]
fn pack() {
    use std::any::type_name;

    let world = World::new();
    let (mut usizes, mut u64s, mut u32s, mut u16s) =
        world.borrow::<(&mut usize, &mut u64, &mut u32, &mut u16)>();

    (&mut usizes, &mut u64s).tight_pack();

    match (&mut usizes, &mut u64s).try_tight_pack() {
        Ok(_) => panic!(),
        Err(err) => assert!(
            err == shipyard::error::Pack::AlreadyTightPack(type_name::<usize>())
                || err == shipyard::error::Pack::AlreadyTightPack(type_name::<u64>())
        ),
    }

    match (&mut usizes, &mut u32s).try_tight_pack() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            err,
            shipyard::error::Pack::AlreadyTightPack(type_name::<usize>())
        ),
    }

    match (&mut u64s, &mut u32s).try_tight_pack() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            err,
            shipyard::error::Pack::AlreadyTightPack(type_name::<u64>())
        ),
    }

    match LoosePack::<(usize, u64)>::try_loose_pack((&mut usizes, &mut u64s, &mut u32s)) {
        Ok(_) => panic!(),
        Err(err) => assert!(
            err == shipyard::error::Pack::AlreadyTightPack(type_name::<usize>())
                || err == shipyard::error::Pack::AlreadyTightPack(type_name::<u64>())
        ),
    }

    match (&mut usizes, &mut u32s).try_loose_pack() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            err,
            shipyard::error::Pack::AlreadyTightPack(type_name::<usize>())
        ),
    }

    match (&mut u64s, &mut u32s).try_loose_pack() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            err,
            shipyard::error::Pack::AlreadyTightPack(type_name::<u64>())
        ),
    }

    LoosePack::<(u32,)>::loose_pack((&mut u32s, &mut usizes, &mut u64s));

    match (&mut u32s, &mut u16s).try_tight_pack() {
        Ok(_) => panic!(),
        Err(err) => assert!(err == shipyard::error::Pack::AlreadyLoosePack(type_name::<u32>())),
    }

    match (&mut u32s, &mut u16s).try_loose_pack() {
        Ok(_) => panic!(),
        Err(err) => assert_eq!(
            err,
            shipyard::error::Pack::AlreadyLoosePack(type_name::<u32>())
        ),
    }
}
