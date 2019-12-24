use super::enumerate::Enumerate;
use super::filter::Filter;
use super::map::Map;
use super::with_id::WithId;

pub trait Shiperator {
    type Item;

    /// # Safety
    ///
    /// `post_process` has to be called with its returned value.
    unsafe fn first_pass(&mut self) -> Option<Self::Item>;
    /// # Safety
    ///
    /// `item` has to come from `first_pass`.
    unsafe fn post_process(&mut self, item: Self::Item) -> Self::Item;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let item = self.first_pass()?;
            Some(self.post_process(item))
        }
    }
    fn for_each<F>(self, f: F)
    where
        Self: Sized,
        F: FnMut(Self::Item),
    {
        #[inline]
        fn call<T>(mut f: impl FnMut(T)) -> impl FnMut((), T) {
            move |(), item| f(item)
        }

        self.fold((), call(f));
    }
    fn try_for_each<F, E>(&mut self, f: F) -> Result<(), E>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Result<(), E>,
    {
        #[inline]
        fn call<T, R>(mut f: impl FnMut(T) -> R) -> impl FnMut((), T) -> R {
            move |(), x| f(x)
        }

        self.try_fold((), call(f))
    }
    fn fold<Acc, F>(mut self, mut acc: Acc, mut f: F) -> Acc
    where
        Self: Sized,
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        while let Some(item) = unsafe { self.first_pass() } {
            acc = f(acc, unsafe { self.post_process(item) });
        }
        acc
    }
    fn try_fold<Acc, F, E>(&mut self, mut acc: Acc, mut f: F) -> Result<Acc, E>
    where
        Self: Sized,
        F: FnMut(Acc, Self::Item) -> Result<Acc, E>,
    {
        while let Some(item) = unsafe { self.first_pass() } {
            acc = f(acc, unsafe { self.post_process(item) })?;
        }
        Ok(acc)
    }
    fn enumerate(self) -> Enumerate<Self>
    where
        Self: Sized,
    {
        Enumerate::new(self)
    }
    fn with_id(self) -> WithId<Self>
    where
        Self: Sized,
    {
        WithId::new(self)
    }
    fn filter<P>(self, pred: P) -> Filter<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        Filter::new(self, pred)
    }
    fn count(self) -> usize
    where
        Self: Sized,
    {
        #[inline]
        fn add1<T>(count: usize, _: T) -> usize {
            std::ops::Add::add(count, 1)
        }

        self.fold(0, add1)
    }
    fn map<R, F>(self, f: F) -> Map<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> R,
    {
        Map::new(self, f)
    }
    fn find<P>(&mut self, pred: P) -> Option<Self::Item>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        #[inline]
        fn check<T>(mut predicate: impl FnMut(&T) -> bool) -> impl FnMut(T) -> Result<(), T> {
            move |x| {
                if predicate(&x) {
                    Err(x)
                } else {
                    Ok(())
                }
            }
        }

        match self.try_for_each(check(pred)) {
            Ok(_) => None,
            Err(item) => Some(item),
        }
    }
}

pub trait CurrentId: Shiperator {
    type Id;

    /// # Safety
    ///
    /// `first_pass` has to be called before calling it.
    unsafe fn current_id(&self) -> Self::Id;
}
