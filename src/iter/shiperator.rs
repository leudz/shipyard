use super::enumerate::Enumerate;
use super::filter::Filter;
use super::map::Map;
use super::with_id::WithId;
use core::iter::FromIterator;

/// Iterator-like trait able to flag only yielded components and not visited ones.
pub trait Shiperator {
    type Item;

    /// `post_process` should be called with its returned value.
    fn first_pass(&mut self) -> Option<Self::Item>;
    fn post_process(&mut self);
    fn size_hint(&self) -> (usize, Option<usize>);
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.first_pass()?;
        self.post_process();
        Some(item)
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
        while let Some(item) = self.first_pass() {
            self.post_process();
            acc = f(acc, item);
        }
        acc
    }
    fn try_fold<Acc, F, E>(&mut self, mut acc: Acc, mut f: F) -> Result<Acc, E>
    where
        Self: Sized,
        F: FnMut(Acc, Self::Item) -> Result<Acc, E>,
    {
        while let Some(item) = self.first_pass() {
            self.post_process();
            acc = f(acc, item)?;
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
            core::ops::Add::add(count, 1)
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
    fn into_iter(self) -> IntoIterator<Self>
    where
        Self: Sized,
    {
        IntoIterator(self)
    }
    fn collect<C: FromIterator<Self::Item>>(self) -> C
    where
        Self: Sized,
    {
        self.into_iter().collect()
    }
}

impl<S: Shiperator + ?Sized> Shiperator for &mut S {
    type Item = <S as Shiperator>::Item;

    fn first_pass(&mut self) -> Option<Self::Item> {
        (**self).first_pass()
    }
    fn post_process(&mut self) {
        (**self).post_process()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }
}

pub trait CurrentId: Shiperator {
    type Id;

    /// # Safety
    ///
    /// `first_pass` has to be called before calling it.
    unsafe fn current_id(&self) -> Self::Id;
}

/// A Shiperator with a known fixed length.
#[allow(clippy::len_without_is_empty)]
pub trait ExactSizeShiperator: Shiperator {
    fn len(&self) -> usize {
        let (lower, upper) = self.size_hint();
        debug_assert!(Some(lower) == upper);
        lower
    }
}

impl<S: ExactSizeShiperator> ExactSizeShiperator for &mut S {}

/// A Shiperator also able to yield item from its tail.
pub trait DoubleEndedShiperator: Shiperator {
    fn first_pass_back(&mut self) -> Option<Self::Item>;
    fn next_back(&mut self) -> Option<Self::Item> {
        let item = self.first_pass_back()?;
        self.post_process();
        Some(item)
    }
}

impl<S: DoubleEndedShiperator> DoubleEndedShiperator for &mut S {
    fn first_pass_back(&mut self) -> Option<Self::Item> {
        (**self).first_pass_back()
    }
}

pub struct IntoIterator<S: ?Sized>(pub(crate) S);

impl<S: Shiperator + ?Sized> Iterator for IntoIterator<S> {
    type Item = <S as Shiperator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<S: ExactSizeShiperator + ?Sized> ExactSizeIterator for IntoIterator<S> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<S: DoubleEndedShiperator + ?Sized> DoubleEndedIterator for IntoIterator<S> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}
