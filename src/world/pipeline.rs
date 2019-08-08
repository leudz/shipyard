use crate::component_storage::AllStorages;
use crate::entity::Entities;
use crate::run::{Dispatch, Mutation, System, SystemData};
use std::any::TypeId;
use std::collections::HashMap;
use std::ops::Range;

pub struct Pipeline {
    pub(super) systems: Vec<Box<dyn Dispatch + Send + Sync>>,
    // a batch list systems running in parallel
    pub(super) batch: Vec<Box<[usize]>>,
    // first usize is the index where the workload begins
    // the second is the number of batch in it
    pub(super) workloads: HashMap<String, Range<usize>>,
    pub(super) default: Range<usize>,
}

impl Default for Pipeline {
    fn default() -> Self {
        Pipeline {
            systems: Vec::new(),
            batch: Vec::new(),
            workloads: HashMap::new(),
            default: 0..0,
        }
    }
}

pub trait Workload {
    fn into_workload(self, name: String, pipeline: &mut Pipeline);
}

impl<T: for<'a> System<'a> + Send + Sync + 'static> Workload for T {
    #[allow(clippy::range_plus_one)]
    fn into_workload(self, name: String, pipeline: &mut Pipeline) {
        if pipeline.workloads.is_empty() {
            pipeline.default = pipeline.batch.len()..(pipeline.batch.len() + 1);
        }
        pipeline
            .workloads
            .insert(name, pipeline.batch.len()..(pipeline.batch.len() + 1));
        pipeline.batch.push(Box::new([pipeline.systems.len()]));
        pipeline.systems.push(Box::new(self));
    }
}

impl<T: for<'a> System<'a> + Send + Sync + 'static> Workload for (T,) {
    fn into_workload(self, name: String, pipeline: &mut Pipeline) {
        self.0.into_workload(name, pipeline)
    }
}

macro_rules! impl_pipeline {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: for<'a> System<'a> + Send + Sync + 'static),+> Workload for ($($type,)+) {
            fn into_workload(self, name: String, pipeline: &mut Pipeline) {
                let batch_start = pipeline.batch.len();
                let mut new_batch: Vec<Vec<usize>> = vec![Vec::new()];
                let mut batch_info: Vec<Vec<(TypeId, Mutation)>> = vec![Vec::new()];

                $({
                    let mut borrow_status = $type::Data::borrow_status();
                    let mut batch_index = batch_info.len();
                    for batch in batch_info.iter().rev() {
                        let mut conflict = false;

                        for &(type_id, mutation) in &borrow_status {
                            match mutation {
                                Mutation::Immutable => {
                                    for &(batch_type_id, mutation) in batch.iter() {
                                        if type_id == batch_type_id && mutation == Mutation::Mutable
                                        || (batch_type_id == TypeId::of::<AllStorages>() && type_id != TypeId::of::<crate::ThreadPool>()) {
                                            conflict = true;
                                            break;
                                        }
                                    };


                                },
                                Mutation::Mutable => {
                                    for &(batch_type_id, _) in batch.iter() {
                                        if type_id == batch_type_id
                                            || (type_id == TypeId::of::<AllStorages>()
                                                && (batch_type_id != TypeId::of::<crate::ThreadPool>()
                                                && batch_type_id != TypeId::of::<Entities>()))
                                        {
                                            conflict = true;
                                            break;
                                        }
                                    };
                                },
                            }
                        }
                        if conflict {
                            break;
                        } else {
                            batch_index -= 1;
                        }
                    }

                    if batch_index == batch_info.len() {
                        new_batch.push(vec![pipeline.systems.len()]);
                        batch_info.push(borrow_status);
                    } else {
                        new_batch[batch_index].push(pipeline.systems.len());
                        batch_info[batch_index].append(&mut borrow_status);
                    }

                    pipeline.systems.push(Box::new(self.$index));
                })+

                pipeline.batch.extend(new_batch.into_iter().map(|batch| batch.into_boxed_slice()));

                if pipeline.workloads.is_empty() {
                    pipeline.default = batch_start..(pipeline.batch.len());
                }

                pipeline.workloads.insert(name, batch_start..(pipeline.batch.len()));
            }
        }
    }
}

macro_rules! pipeline {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_pipeline![$(($type, $index))*];
        pipeline![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_pipeline![$(($type, $index))*];
    }
}

pipeline![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) /*(F, 5) (G, 6) (H, 7) (I, 8) (J, 9) (K, 10) (L, 11)*/];

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn single_immutable() {
        struct System1;
        impl<'a> System<'a> for System1 {
            type Data = (&'a usize,);
            fn run(&self, _: <Self::Data as SystemData>::View) {}
        }

        let mut pipeline = Pipeline::default();
        System1.into_workload("System1".to_string(), &mut pipeline);
        assert_eq!(pipeline.systems.len(), 1);
        assert_eq!(pipeline.batch.len(), 1);
        assert_eq!(&*pipeline.batch[0], &[0]);
        assert_eq!(pipeline.workloads.len(), 1);
        assert_eq!(pipeline.workloads.get("System1"), Some(&(0..1)));
        assert_eq!(pipeline.default, 0..1);
    }
    #[test]
    fn single_mutable() {
        struct System1;
        impl<'a> System<'a> for System1 {
            type Data = (&'a mut usize,);
            fn run(&self, _: <Self::Data as SystemData>::View) {}
        }

        let mut pipeline = Pipeline::default();
        System1.into_workload("System1".to_string(), &mut pipeline);
        assert_eq!(pipeline.systems.len(), 1);
        assert_eq!(pipeline.batch.len(), 1);
        assert_eq!(&*pipeline.batch[0], &[0]);
        assert_eq!(pipeline.workloads.len(), 1);
        assert_eq!(pipeline.workloads.get("System1"), Some(&(0..1)));
        assert_eq!(pipeline.default, 0..1);
    }
    #[test]
    fn multiple_immutable() {
        struct System1;
        impl<'a> System<'a> for System1 {
            type Data = (&'a usize,);
            fn run(&self, _: <Self::Data as SystemData>::View) {}
        }
        struct System2;
        impl<'a> System<'a> for System2 {
            type Data = (&'a usize,);
            fn run(&self, _: <Self::Data as SystemData>::View) {}
        }

        let mut pipeline = Pipeline::default();
        (System1, System2).into_workload("Systems".to_string(), &mut pipeline);
        assert_eq!(pipeline.systems.len(), 2);
        assert_eq!(pipeline.batch.len(), 1);
        assert_eq!(&*pipeline.batch[0], &[0, 1]);
        assert_eq!(pipeline.workloads.len(), 1);
        assert_eq!(pipeline.workloads.get("Systems"), Some(&(0..1)));
        assert_eq!(pipeline.default, 0..1);
    }
    #[test]
    fn multiple_mutable() {
        struct System1;
        impl<'a> System<'a> for System1 {
            type Data = (&'a mut usize,);
            fn run(&self, _: <Self::Data as SystemData>::View) {}
        }
        struct System2;
        impl<'a> System<'a> for System2 {
            type Data = (&'a mut usize,);
            fn run(&self, _: <Self::Data as SystemData>::View) {}
        }

        let mut pipeline = Pipeline::default();
        (System1, System2).into_workload("Systems".to_string(), &mut pipeline);
        assert_eq!(pipeline.systems.len(), 2);
        assert_eq!(pipeline.batch.len(), 2);
        assert_eq!(&*pipeline.batch[0], &[0]);
        assert_eq!(&*pipeline.batch[1], &[1]);
        assert_eq!(pipeline.workloads.len(), 1);
        assert_eq!(pipeline.workloads.get("Systems"), Some(&(0..2)));
        assert_eq!(pipeline.default, 0..2);
    }
    #[test]
    fn multiple_mixed() {
        struct System1;
        impl<'a> System<'a> for System1 {
            type Data = (&'a mut usize,);
            fn run(&self, _: <Self::Data as SystemData>::View) {}
        }
        struct System2;
        impl<'a> System<'a> for System2 {
            type Data = (&'a usize,);
            fn run(&self, _: <Self::Data as SystemData>::View) {}
        }

        let mut pipeline = Pipeline::default();
        (System1, System2).into_workload("Systems".to_string(), &mut pipeline);
        assert_eq!(pipeline.systems.len(), 2);
        assert_eq!(pipeline.batch.len(), 2);
        assert_eq!(&*pipeline.batch[0], &[0]);
        assert_eq!(&*pipeline.batch[1], &[1]);
        assert_eq!(pipeline.workloads.len(), 1);
        assert_eq!(pipeline.workloads.get("Systems"), Some(&(0..2)));
        assert_eq!(pipeline.default, 0..2);

        let mut pipeline = Pipeline::default();
        (System2, System1).into_workload("Systems".to_string(), &mut pipeline);
        assert_eq!(pipeline.systems.len(), 2);
        assert_eq!(pipeline.batch.len(), 2);
        assert_eq!(&*pipeline.batch[0], &[0]);
        assert_eq!(&*pipeline.batch[1], &[1]);
        assert_eq!(pipeline.workloads.len(), 1);
        assert_eq!(pipeline.workloads.get("Systems"), Some(&(0..2)));
        assert_eq!(pipeline.default, 0..2);
    }
    #[test]
    fn all_storages() {
        struct System1;
        impl<'a> System<'a> for System1 {
            type Data = (&'a usize,);
            fn run(&self, _: <Self::Data as SystemData>::View) {}
        }
        struct System2;
        impl<'a> System<'a> for System2 {
            type Data = (AllStorages,);
            fn run(&self, _: <Self::Data as SystemData>::View) {}
        }

        let mut pipeline = Pipeline::default();
        (System1, System2).into_workload("Systems".to_string(), &mut pipeline);
        assert_eq!(pipeline.systems.len(), 2);
        assert_eq!(pipeline.batch.len(), 2);
        assert_eq!(&*pipeline.batch[0], &[0]);
        assert_eq!(&*pipeline.batch[1], &[1]);
        assert_eq!(pipeline.workloads.len(), 1);
        assert_eq!(pipeline.workloads.get("Systems"), Some(&(0..2)));
        assert_eq!(pipeline.default, 0..2);

        let mut pipeline = Pipeline::default();
        (System2, System1).into_workload("Systems".to_string(), &mut pipeline);
        assert_eq!(pipeline.systems.len(), 2);
        assert_eq!(pipeline.batch.len(), 2);
        assert_eq!(&*pipeline.batch[0], &[0]);
        assert_eq!(&*pipeline.batch[1], &[1]);
        assert_eq!(pipeline.workloads.len(), 1);
        assert_eq!(pipeline.workloads.get("Systems"), Some(&(0..2)));
        assert_eq!(pipeline.default, 0..2);
    }
}
