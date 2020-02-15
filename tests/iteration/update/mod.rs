mod multiple;
#[cfg(feature = "parallel")]
#[cfg_attr(miri, ignore)]
mod par_single;
mod single;
