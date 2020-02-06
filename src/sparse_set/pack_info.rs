use crate::storage::EntityId;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::TypeId;
use core::cmp::Ordering;

#[allow(clippy::enum_variant_names)]
pub(crate) enum Pack<T> {
    Tight(TightPack),
    Loose(LoosePack),
    Update(UpdatePack<T>),
    NoPack,
}

impl<T> Pack<T> {
    pub(crate) fn is_loose(&self) -> bool {
        match self {
            Pack::Loose(_) => true,
            _ => false,
        }
    }
}

pub struct PackInfo<T> {
    pub(crate) pack: Pack<T>,
    pub(crate) observer_types: Vec<TypeId>,
}

impl<T> Default for PackInfo<T> {
    fn default() -> Self {
        PackInfo {
            pack: Pack::NoPack,
            observer_types: Vec::new(),
        }
    }
}

impl<T> PackInfo<T> {
    /// `components` is a sorted slice of all types this entity has.
    /// `additional` is a sorted slice of types this entity might have.
    pub(crate) fn check_types(
        &self,
        components: &[TypeId],
        additional: &[TypeId],
    ) -> Result<&[TypeId], ()> {
        let mut self_types: Vec<_> = match &self.pack {
            Pack::Tight(pack) => pack
                .types
                .iter()
                .copied()
                .chain(self.observer_types.iter().copied())
                .collect(),
            Pack::Loose(pack) => pack
                .tight_types
                .iter()
                .copied()
                .chain(pack.loose_types.iter().copied())
                .chain(self.observer_types.iter().copied())
                .collect(),
            Pack::Update(_) => self.observer_types.iter().copied().collect(),
            Pack::NoPack => self.observer_types.iter().copied().collect(),
        };

        self_types.sort_unstable();

        let (all_types, should_pack) = check_types(&self_types, components, additional);
        if all_types && should_pack {
            Ok(match &self.pack {
                Pack::Tight(pack) => &pack.types,
                Pack::Loose(pack) => &pack.tight_types,
                Pack::Update(_) => &[],
                Pack::NoPack => &[],
            })
        } else if all_types {
            Ok(&[])
        } else {
            Err(())
        }
    }
}

pub(crate) struct TightPack {
    pub(crate) types: Arc<[TypeId]>,
    pub(crate) len: usize,
}

impl TightPack {
    pub(crate) fn new(types: Arc<[TypeId]>) -> Self {
        TightPack { types, len: 0 }
    }
    pub(crate) fn check_types(&self, components: &[TypeId]) -> Result<&[TypeId], ()> {
        if check_types(&self.types, &components, &[]) == (true, true) {
            Ok(&self.types)
        } else {
            Err(())
        }
    }
}

pub(crate) struct LoosePack {
    pub(crate) tight_types: Arc<[TypeId]>,
    pub(crate) loose_types: Arc<[TypeId]>,
    pub(crate) len: usize,
}

impl LoosePack {
    pub(crate) fn new(tight_types: Arc<[TypeId]>, loose_types: Arc<[TypeId]>) -> Self {
        LoosePack {
            tight_types,
            loose_types,
            len: 0,
        }
    }
    pub(crate) fn check_all_types(&self, components: &[TypeId]) -> Result<&[TypeId], ()> {
        let mut all_types: Vec<_> = self
            .tight_types
            .iter()
            .copied()
            .chain(self.loose_types.iter().copied())
            .collect();
        all_types.sort_unstable();
        if check_types(&all_types, &components, &[]) == (true, true) {
            Ok(&self.tight_types)
        } else {
            Err(())
        }
    }
}

pub(crate) struct UpdatePack<T> {
    pub(crate) inserted: usize,
    pub(crate) modified: usize,
    pub(crate) deleted: Vec<(EntityId, T)>,
}

/// The first returned `bool` is true if all packed types are present.
/// in either `components` or `additional`.
/// The second returned `bool` is true when all pack types are contained in `components`.
/// `components` is a sorted slice of all types this entity has.
/// `additional` is a sorted slice of types this entity might have.
fn check_types(
    self_types: &[TypeId],
    components: &[TypeId],
    additional: &[TypeId],
) -> (bool, bool) {
    if components.len() + additional.len() < self_types.len() {
        return (false, false);
    }

    let mut packed = 0;
    let mut comp = 0;
    let mut add = 0;

    while packed < self_types.len() {
        if comp < components.len() && add < additional.len() {
            if components[comp] < additional[add] {
                match self_types[packed].cmp(&components[comp]) {
                    Ordering::Greater => comp += 1,
                    Ordering::Equal => {
                        packed += 1;
                        comp += 1;
                    }
                    Ordering::Less => return (false, false),
                }
            } else {
                match self_types[packed].cmp(&additional[add]) {
                    Ordering::Greater => add += 1,
                    Ordering::Equal => {
                        packed += 1;
                        add += 1;
                    }
                    Ordering::Less => return (false, false),
                }
            }
        } else if comp < components.len() {
            match self_types[packed].cmp(&components[comp]) {
                Ordering::Greater => comp += 1,
                Ordering::Equal => {
                    packed += 1;
                    comp += 1;
                }
                Ordering::Less => return (false, false),
            }
        } else if add < additional.len() {
            match self_types[packed].cmp(&additional[add]) {
                Ordering::Greater => add += 1,
                Ordering::Equal => {
                    packed += 1;
                    add += 1;
                }
                Ordering::Less => return (false, false),
            }
        } else {
            break;
        }
    }

    if packed == self_types.len() {
        (true, packed == comp)
    } else {
        (false, false)
    }
}

#[test]
fn pack_check() {
    let pack_types = &mut [
        TypeId::of::<usize>(),
        TypeId::of::<u32>(),
        TypeId::of::<String>(),
    ];
    pack_types.sort_unstable();

    let components = &[];
    let additional = &mut [TypeId::of::<usize>(), TypeId::of::<String>()];
    additional.sort_unstable();
    assert_eq!(
        check_types(pack_types, components, additional),
        (false, false)
    );

    let components = &[];
    let additional = &mut [
        TypeId::of::<usize>(),
        TypeId::of::<i8>(),
        TypeId::of::<String>(),
    ];
    additional.sort_unstable();
    assert_eq!(
        check_types(pack_types, components, additional),
        (false, false)
    );

    let components = &[];
    let additional = &mut [
        TypeId::of::<usize>(),
        TypeId::of::<u32>(),
        TypeId::of::<String>(),
    ];
    additional.sort_unstable();
    assert_eq!(
        check_types(pack_types, components, additional),
        (true, false)
    );

    let components = &[TypeId::of::<usize>()];
    let additional = &mut [TypeId::of::<u32>(), TypeId::of::<String>()];
    additional.sort_unstable();
    assert_eq!(
        check_types(pack_types, components, additional),
        (true, false)
    );

    let components = &mut [
        TypeId::of::<usize>(),
        TypeId::of::<u32>(),
        TypeId::of::<String>(),
    ];
    components.sort_unstable();
    let additional = &mut [];
    assert_eq!(
        check_types(pack_types, components, additional),
        (true, true)
    );
}
