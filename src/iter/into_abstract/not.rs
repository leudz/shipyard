use super::IntoAbstract;
use crate::entity_id::EntityId;
use crate::iter::abstract_mut::AbstractMut;
use crate::not::Not;
use crate::type_id::TypeId;

impl<T: IntoAbstract> IntoAbstract for Not<T>
where
    Not<T::AbsView>: AbstractMut,
{
    type AbsView = Not<T::AbsView>;

    fn into_abstract(self) -> Self::AbsView {
        Not(self.0.into_abstract())
    }
    fn len(&self) -> Option<usize> {
        if self.0.is_tracking() {
            self.0.len()
        } else {
            None
        }
    }
    fn type_id(&self) -> TypeId {
        self.0.type_id()
    }
    fn inner_type_id(&self) -> TypeId {
        self.0.inner_type_id()
    }
    fn dense(&self) -> *const EntityId {
        self.0.dense()
    }
    fn is_not(&self) -> bool {
        true
    }
}
