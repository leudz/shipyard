use crate::sparse_set::SparseArray;
use crate::storage::EntityId;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::TypeId;

pub(crate) const BUCKET_SIZE: usize = 128 / core::mem::size_of::<EntityId>();

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

pub struct Metadata<T> {
    pub(crate) pack: Pack<T>,
    pub(crate) observer_types: Vec<TypeId>,
    pub(crate) shared: SparseArray<[EntityId; BUCKET_SIZE]>,
}

impl<T> Default for Metadata<T> {
    fn default() -> Self {
        Metadata {
            pack: Pack::NoPack,
            observer_types: Vec::new(),
            shared: SparseArray::new(),
        }
    }
}

impl<T> Metadata<T> {
    /// Returns `true` if enough storages were passed in
    pub(crate) fn has_all_storages(&self, components: &[TypeId], additionals: &[TypeId]) -> bool {
        match &self.pack {
            Pack::Tight(tight) => {
                tight.has_all_storages(components, additionals, &self.observer_types)
            }
            Pack::Loose(loose) => {
                loose.has_all_storages(components, additionals, &self.observer_types)
            }
            Pack::Update(_) | Pack::NoPack => {
                if components.len() + additionals.len() < self.observer_types.len() {
                    return false;
                }

                // current component index
                let mut comp = 0;
                // current additional index
                let mut add = 0;

                // we know observer types are at most as many as components + additionals so we'll use them to drive the iteration
                for &observer_type in &self.observer_types {
                    // we skip components with a lower TypeId
                    comp += components[comp..]
                        .iter()
                        .take_while(|&&component| component < observer_type)
                        .count();

                    // we also skip additional types with a lower TypeId
                    add += additionals[add..]
                        .iter()
                        .take_while(|&&additional| additional < observer_type)
                        .count();

                    // one of them has to be equal to observer_type else not enough storages where passed in
                    match (components.get(comp), additionals.get(add)) {
                        (Some(&component), Some(&additional))
                            if component == observer_type || additional == observer_type => {}
                        (Some(&component), None) if component == observer_type => {}
                        (None, Some(&additional)) if additional == observer_type => {}
                        _ => return false,
                    }
                }

                true
            }
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
    /// Returns `Ok(packed_types)` if `components` contains at least all components in `self.types`
    pub(crate) fn is_packable(&self, components: &[TypeId]) -> Result<&[TypeId], ()> {
        // the entity doesn't have enough components to be packed
        if components.len() < self.types.len() {
            return Err(());
        }

        // current component index
        let mut comp = 0;

        // we know packed types are at most as many as components so we'll use them to drive the iteration
        for &packed_type in &*self.types {
            // we skip components with a lower TypeId
            comp += components[comp..]
                .iter()
                .take_while(|&&component| component < packed_type)
                .count();

            // since both slices are sorted, if the types aren't equal it means components is missing a packed type
            if components
                .get(comp)
                .filter(|&&component| component == packed_type)
                .is_none()
            {
                return Err(());
            }
        }
        Ok(&self.types)
    }
    /// Returns `true` if enough storages were passed in
    fn has_all_storages(
        &self,
        components: &[TypeId],
        additionals: &[TypeId],
        observer_types: &[TypeId],
    ) -> bool {
        // both pairs can't have duplicates
        if components.len() + additionals.len() < self.types.len() + observer_types.len() {
            return false;
        }

        // current tight type
        let mut tight = 0;
        // current observer type
        let mut observer = 0;
        // current component
        let mut comp = 0;
        // current additional
        let mut add = 0;

        // we use the tight and observer types to drive the iteration since there are at most the same count as components + additionals
        loop {
            // since both arrays are sorted and a value can't be in both we can iterate just once
            // but we have to make sure to not stop the iteration too early when tight or loose ends
            match (self.types.get(tight), observer_types.get(observer)) {
                (Some(&tight_type), observer_type)
                    if observer_type.is_none() || tight_type < *observer_type.unwrap() =>
                {
                    // we skip components with a lower TypeId
                    comp += components[comp..]
                        .iter()
                        .take_while(|&&component| component < tight_type)
                        .count();

                    // we also skip additional types with a lower TypeId
                    add += additionals[add..]
                        .iter()
                        .take_while(|&&additional| additional < tight_type)
                        .count();

                    // one of them has to be equal to tight_type else not enough storages where passed in
                    // we also have to update the number of components found in the tight_types
                    match (components.get(comp), additionals.get(add)) {
                        (Some(&component), Some(&additional))
                            if component == tight_type || additional == tight_type =>
                        {
                            tight += 1
                        }
                        (Some(&component), None) if component == tight_type => tight += 1,
                        (None, Some(&additional)) if additional == tight_type => tight += 1,
                        _ => return false,
                    }
                }
                (Some(_), None) => unreachable!(), // the compiler isn't smart enough to see this
                (_, Some(&observer_type)) => {
                    comp += components[comp..]
                        .iter()
                        .take_while(|&&component| component < observer_type)
                        .count();
                    add += additionals[add..]
                        .iter()
                        .take_while(|&&additional| additional < observer_type)
                        .count();

                    match (components.get(comp), additionals.get(add)) {
                        (Some(&component), Some(&additional))
                            if component == observer_type || additional == observer_type =>
                        {
                            observer += 1
                        }
                        (Some(&component), None) if component == observer_type => observer += 1,
                        (None, Some(&additional)) if additional == observer_type => observer += 1,
                        _ => return false,
                    }
                }
                (None, None) => break,
            }
        }

        // we check all types were passed in
        tight == self.types.len() && observer == observer_types.len()
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
    /// Returns `Ok(packed_types)` if `components` contains at least all components in `self.types`
    pub(crate) fn is_packable(&self, components: &[TypeId]) -> Result<&[TypeId], ()> {
        if components.len() < self.tight_types.len() + self.loose_types.len() {
            // the entity doesn't have enough components to be packed
            return Err(());
        }

        // current tight type
        let mut tight = 0;
        // current loose type
        let mut loose = 0;
        // current component
        let mut comp = 0;

        // we use the packed types to drive the iteration since there are at most the same count as components
        loop {
            // since both arrays are sorted and a value can't be in both we can iterate just once
            // but we have to make sure to not stop the iteration too early when tight or loose ends
            match (self.tight_types.get(tight), self.loose_types.get(loose)) {
                (Some(&tight_type), loose_type)
                    if loose_type.is_none() || tight_type < *loose_type.unwrap() =>
                {
                    // we skip components with a lower TypeId
                    comp += components[comp..]
                        .iter()
                        .take_while(|&&component| component < tight_type)
                        .count();

                    if components
                        .get(comp)
                        .filter(|&&component| component == tight_type)
                        .is_some()
                    {
                        tight += 1;
                    } else {
                        return Err(());
                    }
                }
                (Some(_), None) => unreachable!(),
                (_, Some(&loose_type)) => {
                    comp += components[comp..]
                        .iter()
                        .take_while(|&&component| component < loose_type)
                        .count();

                    if components
                        .get(comp)
                        .filter(|&&component| component == loose_type)
                        .is_some()
                    {
                        loose += 1;
                    } else {
                        return Err(());
                    }
                }
                (None, None) => break,
            }
        }

        if tight == self.tight_types.len() && loose == self.loose_types.len() {
            Ok(&self.tight_types)
        } else {
            Err(())
        }
    }
    #[allow(clippy::cognitive_complexity)]
    /// Returns `true` if enough storages were passed in
    fn has_all_storages(
        &self,
        components: &[TypeId],
        additionals: &[TypeId],
        observer_types: &[TypeId],
    ) -> bool {
        if components.len() + additionals.len()
            < self.tight_types.len() + self.loose_types.len() + observer_types.len()
        {
            return false;
        }

        // current tight type
        let mut tight = 0;
        // current loose type
        let mut loose = 0;
        // current observer type
        let mut observer = 0;
        // current component
        let mut comp = 0;
        // current additional
        let mut add = 0;

        // we use the packed types to drive the iteration since there are at most the same count as components
        loop {
            // since both arrays are sorted and a value can't be in both we can iterate just once
            // but we have to make sure to not stop the iteration too early when tight or loose ends
            match (
                self.tight_types.get(tight),
                self.loose_types.get(loose),
                observer_types.get(observer),
            ) {
                (Some(&tight_type), Some(&loose_type), Some(&observer_type)) => {
                    if tight_type < loose_type && tight_type < observer_type {
                        // we skip components with a lower TypeId
                        comp += components[comp..]
                            .iter()
                            .take_while(|&&component| component < tight_type)
                            .count();

                        // we also skip additional types with a lower TypeId
                        add += additionals[add..]
                            .iter()
                            .take_while(|&&additional| additional < tight_type)
                            .count();

                        // one of them has to be equal to tight_type else not enough storages where passed in
                        // we also have to update the number of components found in the tight_types
                        match (components.get(comp), additionals.get(add)) {
                            (Some(&component), Some(&additional))
                                if component == tight_type || additional == tight_type =>
                            {
                                tight += 1
                            }
                            (Some(&component), None) if component == tight_type => tight += 1,
                            (None, Some(&additional)) if additional == tight_type => tight += 1,
                            _ => return false,
                        }
                    } else if loose_type < observer_type {
                        comp += components[comp..]
                            .iter()
                            .take_while(|&&component| component < loose_type)
                            .count();
                        add += additionals[add..]
                            .iter()
                            .take_while(|&&additional| additional < loose_type)
                            .count();

                        match (components.get(comp), additionals.get(add)) {
                            (Some(&component), Some(&additional))
                                if component == loose_type || additional == loose_type =>
                            {
                                loose += 1
                            }
                            (Some(&component), None) if component == loose_type => loose += 1,
                            (None, Some(&additional)) if additional == loose_type => loose += 1,
                            _ => return false,
                        }
                    } else {
                        comp += components[comp..]
                            .iter()
                            .take_while(|&&component| component < observer_type)
                            .count();
                        add += additionals[add..]
                            .iter()
                            .take_while(|&&additional| additional < observer_type)
                            .count();

                        match (components.get(comp), additionals.get(add)) {
                            (Some(&component), Some(&additional))
                                if component == observer_type || additional == observer_type =>
                            {
                                observer += 1
                            }
                            (Some(&component), None) if component == observer_type => observer += 1,
                            (None, Some(&additional)) if additional == observer_type => {
                                observer += 1
                            }
                            _ => return false,
                        }
                    }
                }
                (Some(&tight_type), Some(&loose_type), None) => {
                    if tight_type < loose_type {
                        comp += components[comp..]
                            .iter()
                            .take_while(|&&component| component < tight_type)
                            .count();
                        add += additionals[add..]
                            .iter()
                            .take_while(|&&additional| additional < tight_type)
                            .count();

                        match (components.get(comp), additionals.get(add)) {
                            (Some(&component), Some(&additional))
                                if component == tight_type || additional == tight_type =>
                            {
                                tight += 1
                            }
                            (Some(&component), None) if component == tight_type => tight += 1,
                            (None, Some(&additional)) if additional == tight_type => tight += 1,
                            _ => return false,
                        }
                    } else {
                        comp += components[comp..]
                            .iter()
                            .take_while(|&&component| component < loose_type)
                            .count();
                        add += additionals[add..]
                            .iter()
                            .take_while(|&&additional| additional < loose_type)
                            .count();

                        match (components.get(comp), additionals.get(add)) {
                            (Some(&component), Some(&additional))
                                if component == loose_type || additional == loose_type =>
                            {
                                loose += 1
                            }
                            (Some(&component), None) if component == loose_type => loose += 1,
                            (None, Some(&additional)) if additional == loose_type => loose += 1,
                            _ => return false,
                        }
                    }
                }
                (Some(&tight_type), None, Some(&observer_type)) => {
                    if tight_type < observer_type {
                        comp += components[comp..]
                            .iter()
                            .take_while(|&&component| component < tight_type)
                            .count();
                        add += additionals[add..]
                            .iter()
                            .take_while(|&&additional| additional < tight_type)
                            .count();

                        match (components.get(comp), additionals.get(add)) {
                            (Some(&component), Some(&additional))
                                if component == tight_type || additional == tight_type =>
                            {
                                tight += 1
                            }
                            (Some(&component), None) if component == tight_type => tight += 1,
                            (None, Some(&additional)) if additional == tight_type => tight += 1,
                            _ => return false,
                        }
                    } else {
                        comp += components[comp..]
                            .iter()
                            .take_while(|&&component| component < observer_type)
                            .count();
                        add += additionals[add..]
                            .iter()
                            .take_while(|&&additional| additional < observer_type)
                            .count();

                        match (components.get(comp), additionals.get(add)) {
                            (Some(&component), Some(&additional))
                                if component == observer_type || additional == observer_type =>
                            {
                                observer += 1
                            }
                            (Some(&component), None) if component == observer_type => observer += 1,
                            (None, Some(&additional)) if additional == observer_type => {
                                observer += 1
                            }
                            _ => return false,
                        }
                    }
                }
                (None, Some(&loose_type), Some(&observer_type)) => {
                    if loose_type < observer_type {
                        comp += components[comp..]
                            .iter()
                            .take_while(|&&component| component < loose_type)
                            .count();
                        add += additionals[add..]
                            .iter()
                            .take_while(|&&additional| additional < loose_type)
                            .count();

                        match (components.get(comp), additionals.get(add)) {
                            (Some(&component), Some(&additional))
                                if component == loose_type || additional == loose_type =>
                            {
                                loose += 1
                            }
                            (Some(&component), None) if component == loose_type => loose += 1,
                            (None, Some(&additional)) if additional == loose_type => loose += 1,
                            _ => return false,
                        }
                    } else {
                        comp += components[comp..]
                            .iter()
                            .take_while(|&&component| component < observer_type)
                            .count();
                        add += additionals[add..]
                            .iter()
                            .take_while(|&&additional| additional < observer_type)
                            .count();

                        match (components.get(comp), additionals.get(add)) {
                            (Some(&component), Some(&additional))
                                if component == observer_type || additional == observer_type =>
                            {
                                observer += 1
                            }
                            (Some(&component), None) if component == observer_type => observer += 1,
                            (None, Some(&additional)) if additional == observer_type => {
                                observer += 1
                            }
                            _ => return false,
                        }
                    }
                }
                (Some(&tight_type), None, None) => {
                    comp += components[comp..]
                        .iter()
                        .take_while(|&&component| component < tight_type)
                        .count();
                    add += additionals[add..]
                        .iter()
                        .take_while(|&&additional| additional < tight_type)
                        .count();

                    match (components.get(comp), additionals.get(add)) {
                        (Some(&component), Some(&additional))
                            if component == tight_type || additional == tight_type =>
                        {
                            tight += 1
                        }
                        (Some(&component), None) if component == tight_type => tight += 1,
                        (None, Some(&additional)) if additional == tight_type => tight += 1,
                        _ => return false,
                    }
                }
                (None, Some(&loose_type), None) => {
                    comp += components[comp..]
                        .iter()
                        .take_while(|&&component| component < loose_type)
                        .count();
                    add += additionals[add..]
                        .iter()
                        .take_while(|&&additional| additional < loose_type)
                        .count();

                    match (components.get(comp), additionals.get(add)) {
                        (Some(&component), Some(&additional))
                            if component == loose_type || additional == loose_type =>
                        {
                            loose += 1
                        }
                        (Some(&component), None) if component == loose_type => loose += 1,
                        (None, Some(&additional)) if additional == loose_type => loose += 1,
                        _ => return false,
                    }
                }
                (None, None, Some(&observer_type)) => {
                    comp += components[comp..]
                        .iter()
                        .take_while(|&&component| component < observer_type)
                        .count();
                    add += additionals[add..]
                        .iter()
                        .take_while(|&&additional| additional < observer_type)
                        .count();

                    match (components.get(comp), additionals.get(add)) {
                        (Some(&component), Some(&additional))
                            if component == observer_type || additional == observer_type =>
                        {
                            observer += 1
                        }
                        (Some(&component), None) if component == observer_type => observer += 1,
                        (None, Some(&additional)) if additional == observer_type => observer += 1,
                        _ => return false,
                    }
                }
                (None, None, None) => break,
            }
        }

        tight == self.tight_types.len()
            && loose == self.loose_types.len()
            && observer == observer_types.len()
    }
}

pub(crate) struct UpdatePack<T> {
    pub(crate) inserted: usize,
    pub(crate) modified: usize,
    pub(crate) removed: Vec<EntityId>,
    pub(crate) deleted: Vec<(EntityId, T)>,
}

impl<T> UpdatePack<T> {
    pub(crate) fn first_non_mut(&self) -> usize {
        self.inserted + self.modified
    }
}
