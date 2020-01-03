use super::Scheduler;
use crate::error;
use crate::World;
use std::borrow::Cow;

pub trait IntoWorkloadFn {
    fn into_workload(self, name: impl Into<Cow<'static, str>>, scheduler: &mut Scheduler);
}

impl<T: for<'a> Fn(&'a World) -> Result<(), error::GetStorage> + Send + Sync + 'static>
    IntoWorkloadFn for T
{
    fn into_workload(self, name: impl Into<Cow<'static, str>>, scheduler: &mut Scheduler) {
        let range = scheduler.batch.len()..(scheduler.batch.len() + 1);
        if scheduler.workloads.is_empty() {
            scheduler.default = range.clone();
        }
        scheduler.workloads.insert(name.into(), range);
        scheduler.batch.push(Box::new([scheduler.systems.len()]));
        scheduler.systems.push(Box::new(self));
    }
}

impl<T: for<'a> Fn(&'a World) -> Result<(), error::GetStorage> + Send + Sync + 'static>
    IntoWorkloadFn for (T,)
{
    fn into_workload(self, name: impl Into<Cow<'static, str>>, scheduler: &mut Scheduler) {
        self.0.into_workload(name, scheduler)
    }
}

macro_rules! impl_scheduler {
    ($(($type: ident, $index: tt))+) => {
        impl<$($type: for<'a> Fn(&'a World) -> Result<(), error::GetStorage> + Send + Sync + 'static),+> IntoWorkloadFn for ($($type,)+) {
            fn into_workload(self, name: impl Into<Cow<'static, str>>, scheduler: &mut Scheduler) {
                let start = scheduler.batch.len();
                $(
                    scheduler.batch.push(Box::new([scheduler.systems.len()]));
                    scheduler.systems.push(Box::new(self.$index));
                )+
                let range = start..scheduler.batch.len();
                if scheduler.workloads.is_empty() {
                    scheduler.default = range.clone();
                }
                scheduler.workloads.insert(name.into(), range);
            }
        }
    }
}

macro_rules! scheduler {
    ($(($type: ident, $index: tt))*;($type1: ident, $index1: tt) $(($queue_type: ident, $queue_index: tt))*) => {
        impl_scheduler![$(($type, $index))*];
        scheduler![$(($type, $index))* ($type1, $index1); $(($queue_type, $queue_index))*];
    };
    ($(($type: ident, $index: tt))*;) => {
        impl_scheduler![$(($type, $index))*];
    }
}

scheduler![(A, 0) (B, 1); (C, 2) (D, 3) (E, 4) (F, 5) (G, 6) (H, 7) (I, 8) (J, 9)];
