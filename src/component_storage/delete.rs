use crate::sparse_array::SparseArray;

// When removing an entity all its components have to be removed.
// These components are stored in HashMap<TypeId, Box<dyn Any>> to be able to store multiple types in the HashMap.
// However we can't get back the concrete type of Any and delete the component.
// This is where this trait comes into play.
// It is implemented for all SparseArray and a specific version will be created by the compiler when needed.
// We then store the vtable part of the trait object specific for the type added to the World.
// We do so by taking a pointer to the trait object's fat pointer,
// cast it to *const [usize; 2] and storring the second usize.
// This happens in ComponentStorage::new.
// Then when we want to delete a component we re-assemble the trait object at runtime.
// We do so by taking a reference to the SparseArray, cast it to a thin pointer.
// Put it in an array with the stored vtable pointer and take a pointer to this array.
// The pointer is then cast to a reference to the fat pointer of a Delete trait object.
// This happens in ComponentStorage::delete.
//
// All of this is temporary, the std will provide a way to get the vtable of a trait object
// in the future. Until then this hack works as long as trait objects' fat pointer don't
// change representation.
pub(super) trait Delete {
    fn delete(&mut self, index: usize);
}

impl<T> Delete for SparseArray<T> {
    fn delete(&mut self, index: usize) {
        self.remove(index);
    }
}
