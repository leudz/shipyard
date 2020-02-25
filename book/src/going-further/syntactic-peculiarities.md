# Syntactic Peculiarities

Some parts of Rust are less used than others, let's look into some of them.

### Tuples

Shipyard uses tuples a lot, sometimes even for a single element. There are two main reasons:
- the elements could be any type including a tuple, for example `loose_pack`. If `T` was allowed as input type then `T` could be `(usize, u32)`, that's not what we want though, we want them to be two different elements.
- the compiler can't read our mind (yet), for example `remove`. `Remove` is implemented for up to ten storages but we can `remove` up to the number of storages we passed. We might have to pass 5 storages to the function because they are packed but only want to `remove` from 2.

### Double `mut`

So what about with the double `mut` in systems definitions (e.g. **mut** empties: &**mut** Empty)?  

1. We need to tell Rust that we want a mutable binding, to take a mutable reference for example, this is the first `mut`.
2. We need to tell Shipyard that we want unique access to this storage (`ViewMut`, as opposed to `View`), this is the second `mut`.

### `ref mut`

Some functions, like `get`, can return an immutable or mutable result based on which reference you use and default to immutable if you don't specify.  
Let's look at an example:
```rust, noplaypen
#[system(RefMut)]
fn run(mut usizes: &mut usize) {
    let result: Result<&usize, _> = usizes.get(id);
}
```
We can force the compiler to give us a `&mut usize` with a few methods, one of them is to use `ref mut`:
```rust, noplaypen
#[system(RefMut)]
fn run(ref mut usizes: &mut usize) {
    let result: Result<&usize, _> = usizes.get(id);
}
```

### `ref`

Since Rust will default to immutable return by default, we don't have to use `ref` for the same reason as `ref mut` but we can use it for something else:
```rust, noplaypen
#[system(Ref)]
fn run(usizes: &usize, u32s: &u32) {
    (usizes, u32s).get(id);
}
```
This example doesn't compile since `get` has to use references. We could take the views by reference at each call of course but when we'd have a bunch of them:
```rust, noplaypen
#[system(Ref)]
fn run(usizes: &usize, u32s: &u32) {
    (&usizes, &u32s).get(id);
    (&usizes, &u32s).get(id);
    (&usizes, &u32s).get(id);
    (&usizes, &u32s).get(id);
}
```
It can be easier to just use `ref`:
```
#[system(Ref)]
fn run(ref usizes: &usize, ref u32s: &u32) {
    (usizes, u32s).get(id);
    (usizes, u32s).get(id);
    (usizes, u32s).get(id);
    (usizes, u32s).get(id);
}
```
