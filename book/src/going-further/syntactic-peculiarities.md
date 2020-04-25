# Syntactic Peculiarities

Some parts of Rust are less-used than others, let's look into some of them.

### Tuples

Shipyard uses tuples a lot, sometimes even for a single element. There are two main reasons:
- the elements could be any type including a tuple, for example [`loose_pack`](https://docs.rs/shipyard/latest/shipyard/trait.LoosePack.html#tymethod.loose_pack). If `T` was allowed as input type then `T` could be `(usize, u32)`, that's not what we want though, we want them to be two different elements.
- the compiler can't read our mind (yet), for example [`remove`](https://docs.rs/shipyard/latest/shipyard/trait.Remove.html#tymethod.remove). [`Remove`](https://docs.rs/shipyard/latest/shipyard/trait.Remove.html) is implemented for up to ten storages but we can [`remove`](https://docs.rs/shipyard/latest/shipyard/trait.Remove.html#tymethod.remove) up to the number of storages we passed. We might have to pass 5 storages to the function because they are packed but only want to [`remove`](https://docs.rs/shipyard/latest/shipyard/trait.Remove.html#tymethod.remove) from 2.

### `ref mut`

Some functions, like [`get`](https://docs.rs/shipyard/latest/shipyard/trait.Get.html#tymethod.get), can return an immutable or mutable result based on which reference you use and default to immutable if you don't specify.  
Let's look at an example:
```rust, noplaypen
fn ref_mut(mut u32s: ViewMut<u32>) {
    let i: &u32 = u32s.get(id);
}
```
We can force the compiler to give us a `&mut u32` with a few methods, one of them is to use `ref mut`:
```rust, noplaypen
fn ref_mut(ref mut u32s: ViewMut<u32>) {
    let i: &mut u32 = u32s.get(id);
}
```

### `ref`

Since Rust will default to immutable return by default, we don't have to use `ref` for the same reason as `ref mut` but we can use it for something else:
```rust, noplaypen
fn ref_sys(u32s: View<u32>, usizes: View<usize>) {
    (usizes, u32s).get(id);
}
```
This example doesn't compile since `get` has to use references. We could take the views by reference at each call of course but when we'd have a bunch of them:
```rust, noplaypen
fn ref_sys(u32s: View<u32>, usizes: View<usize>) {
    (&usizes, &u32s).get(id);
    (&usizes, &u32s).get(id);
    (&usizes, &u32s).get(id);
    (&usizes, &u32s).get(id);
}
```
It can be easier to just use `ref`:
```rust, noplaypen
fn ref_sys(ref u32s: View<u32>, ref usizes: View<usize>) {
    (usizes, u32s).get(id);
    (usizes, u32s).get(id);
    (usizes, u32s).get(id);
    (usizes, u32s).get(id);
}
```
