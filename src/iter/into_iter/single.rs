use super::{IntoAbstract, IntoIter, Iter1, Tight1, Update1};
#[cfg(feature = "parallel")]
use super::{ParIter1, ParTight1, ParUpdate1};
use crate::sparse_set::Pack;

impl<T: IntoAbstract> IntoIter for T {
    type IntoIter = Iter1<Self>;
    #[cfg(feature = "parallel")]
    type IntoParIter = ParIter1<Self>;
    fn iter(self) -> Self::IntoIter {
        match &self.metadata().pack {
            Pack::Update(_) => Iter1::Update(Update1::new(self)),
            _ => Iter1::Tight(Tight1::new(self)),
        }
    }
    #[cfg(feature = "parallel")]
    fn par_iter(self) -> Self::IntoParIter {
        match self.iter() {
            Iter1::Tight(tight) => ParTight1::from(tight).into(),
            Iter1::Update(update) => ParUpdate1::from(update).into(),
        }
    }
}

impl<T: IntoIter> IntoIter for (T,) {
    type IntoIter = T::IntoIter;
    #[cfg(feature = "parallel")]
    type IntoParIter = T::IntoParIter;
    fn iter(self) -> Self::IntoIter {
        self.0.iter()
    }
    #[cfg(feature = "parallel")]
    fn par_iter(self) -> Self::IntoParIter {
        self.0.par_iter()
    }
}
