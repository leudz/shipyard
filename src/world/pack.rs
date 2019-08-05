pub trait WorldOwnedPack<'a> {
    type Storage;
}

macro_rules! impl_owned_pack {
    ($(($type: ident, $index: tt))+) => {
        impl<'a, $($type: 'a),+> WorldOwnedPack<'a> for ($($type,)+) {
            type Storage = ($(&'a mut $type,)+);
        }
    }
}

macro_rules! owned_pack {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_owned_pack![$(($type, $index))*];
        owned_pack![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_owned_pack![$(($type, $index))*];
    }
}

owned_pack![(A, 0); (B, 1) (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];
