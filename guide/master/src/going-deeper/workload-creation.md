# Workload creation

There are a few trickeries going on with workload's creation.\
In this chapter we'll look under the hood to understand how shipyard accept:

```rs
Workload::builder("Add & Check")
    .with_system(add);
```

## IntoBorrow

Let's start with `Workload::with_system`.\
It should accept any system, a system being a function with 0 to 10 views as arguments and returning anything.\
Since it has to accept multiple types we have to make a trait, `IntoWorkloadSystem`.\
Ideally this trait would be implemented like this:

```rs
trait Borrow {
    type View<'v>;
    fn borrow<'a>(world: &'a World) -> Result<Self::View<'a>, error::GetStorage>;
}

impl<$($view: Borrow + BorrowInfo,)+ R, Sys> IntoWorkloadSystem<($($view,)+), R> for Sys
where
    Sys:
        Fn($($view),+) -> R
        + 'static
        + Send
        + Sync {
```

But GAT are not stable so we can't have `View<'v>` as an associated type.
Today we have to write:

```rs
trait Borrow<'v> {
    type View;
    fn borrow(world: &'v World) -> Result<Self::View, error::GetStorage>;
}
```

Then `IntoWorkloadSystem` becomes:

```rs
impl<$($view: for<'v> Borrow<'v> + BorrowInfo,)+ R, Sys> IntoWorkloadSystem<($($view,)+), R> for Sys
where
    Sys:
        Fn($($view),+) -> R
        + 'static
        + Send
        + Sync {
```

But the compiler isn't happy.\
At the end of the day, views don't implement `Borrow` for all lifetimes. Views only implement `Borrow` for their lifetime.\
For example `View<'a, T>` will only implement `Borrow<'a>`, if you try `Borrow<'b>` it shouldn't work.

And you can see it with `()`, the unit type actually implements `Borrow` for all lifetimes and the compiler will accept functions that take a unit as argument.

So instead we don't make a single `Borrow` trait, but 2:
- `IntoBorrow` has the ability to give us a type that implements `Borrow` for all lifetimes
- `Borrow` will use this type and give us the actual view
Then we can tie both lifetimes to make it valid.

```rs
impl<$($view: IntoBorrow + BorrowInfo,)+ R, Sys> IntoWorkloadSystem<($($view,)+), R> for Sys
where
    for<'s> Sys:
        Fn($($view),+) -> R
        + Fn($(<$view::Borrow as Borrow<'s>>::View),+) -> R
        + 'static
        + Send
        + Sync {
```

`IntoBorrow` instead of `for<'a> Borrow<'a>` and the two bounds on `Sys` will tie the lifetime of the views from the function's arguments (`'s`) to the views returned by `Borrow`.

## Reference

If you implement `IntoWorkloadSystem` like shown above you'll notice that it works but only for references to functions.
I don't know why, so the real implementation is:

```rs
impl<$($view: IntoBorrow + BorrowInfo,)+ R, Sys> IntoWorkloadSystem<($($view,)+), R> for Sys
where
    Sys: 'static
        + Send
        + Sync,
    for<'a, 'b> &'b Sys:
        Fn($($view),+) -> R
        + Fn($(<$view::Borrow as Borrow<'a>>::View),+) -> R {
```

We take the system as value and make sure it's `'static + Send + Sync` then in `IntoWorkloadSystem` implementation we'll take a reference to it.
