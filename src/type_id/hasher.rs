use core::hash::Hasher;

/// Since `TypeId`s are unique no need to hash them.  
/// This is the purpose of this hasher, not doing anything.  
/// It will get bytes, check if the number is right and return a `u64`.
#[derive(Default)]
pub(crate) struct TypeIdHasher(u64);

impl Hasher for TypeIdHasher {
    fn write(&mut self, bytes: &[u8]) {
        self.0 = u64::from_ne_bytes(bytes.try_into().unwrap());
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(feature = "std")]
#[test]
fn hasher() {
    fn verify<T: 'static + ?Sized>() {
        use core::any::TypeId;
        use core::hash::Hash;
        use core::mem::size_of;

        let type_id = TypeId::of::<T>();

        match size_of::<TypeId>() {
            8 => {
                let mut hasher = TypeIdHasher::default();
                type_id.hash(&mut hasher);

                let type_id: *const _ = &type_id;
                let type_id = type_id as *const u64;
                let type_id = unsafe { *type_id };

                assert_eq!(hasher.finish(), type_id);
            }
            16 => {
                // TypeIdHasher is not used in this case
            }
            _ => panic!("Compiler version not supported"),
        }
    }

    verify::<usize>();
    verify::<()>();
    verify::<str>();
    verify::<&'static str>();
    verify::<[u8; 20]>();
}
