use super::{IntoAbstract, IntoIter, Iter1, Tight1, Update1};
#[cfg(feature = "parallel")]
use super::{ParIter1, ParTight1, ParUpdate1};
use crate::entity::Key;
use crate::sparse_set::Pack;

impl<T: IntoAbstract> IntoIter for T {
    type IntoIter = Iter1<Self>;
    #[cfg(feature = "parallel")]
    type IntoParIter = ParIter1<Self>;
    fn iter(self) -> Self::IntoIter {
        match &self.pack_info().pack {
            Pack::Update(_) => {
                let end = self.len().unwrap_or(0);
                // last_id is never read
                Iter1::Update(Update1 {
                    end,
                    data: self.into_abstract(),
                    current: 0,
                    last_id: Key::dead(),
                })
            }
            _ => Iter1::Tight(Tight1 {
                end: self.len().unwrap_or(0),
                data: self.into_abstract(),
                current: 0,
            }),
        }
    }
    #[cfg(feature = "parallel")]
    fn par_iter(self) -> Self::IntoParIter {
        match self.iter() {
            Iter1::Tight(iter) => ParIter1::Tight(ParTight1(iter)),
            Iter1::Update(iter) => ParIter1::Update(ParUpdate1(iter)),
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
