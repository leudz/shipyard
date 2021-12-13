use super::IntoAbstract;
use crate::entity_id::EntityId;
use crate::iter::abstract_mut::AbstractMut;
use crate::or::Or;
use crate::type_id::TypeId;
use alloc::vec;
use alloc::vec::Vec;

impl<'a: 'b, 'b, T: IntoAbstract, U: IntoAbstract> IntoAbstract for Or<(T, U)>
where
    <<U as IntoAbstract>::AbsView as AbstractMut>::Index: Into<usize> + Clone,
{
    type AbsView = Or<(T::AbsView, U::AbsView)>;

    fn into_abstract(self) -> Self::AbsView {
        Or(((self.0).0.into_abstract(), (self.0).1.into_abstract()))
    }
    fn len(&self) -> Option<usize> {
        (self.0).0.len()
    }
    fn type_id(&self) -> TypeId {
        TypeId::of::<Or<()>>()
    }
    #[inline]
    fn inner_type_id(&self) -> TypeId {
        TypeId::of::<()>()
    }
    fn dense(&self) -> *const EntityId {
        (self.0).0.dense()
    }
    fn other_dense(&self) -> Vec<core::slice::Iter<'static, EntityId>> {
        let slice =
            unsafe { core::slice::from_raw_parts((self.0).1.dense(), (self.0).1.len().unwrap()) };

        vec![slice.iter()]
    }
    fn is_or(&self) -> bool {
        true
    }
}
