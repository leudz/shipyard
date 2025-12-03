# Custom Views

Custom views are types that you can borrow (like `View` or `UniqueView`) but are not provided by `shipyard`.

Many types can become custom views, they'll fall into one of two categories: View Bundle or Wild View.
View bundles only contain other views while wild views can contain other types.

Example of a View Bundle:
```rust, noplaypen
{{#include ../../../../tests/book/custom_view.rs:view_bundle}}
```

Example of a Wild View:
```rust, noplaypen
{{#include ../../../../tests/book/custom_view.rs:wild_view}}
```

### Iteration

View bundles can be iterated directly by deriving the `IntoIter` trait.

```rust, noplaypen
{{#include ../../../../tests/book/custom_view.rs:into_iter}}
```

All attributes are optional.

## Concrete example

When creating a frame with any low level api there is always some boilerplate. We'll look at how custom views can help for `wgpu`.

The original code creates the frame in a system by borrowing `Graphics` which contains everything needed.\
The rendering part just clears the screen with a color.

The entire starting code for this chapter is available in [this file](./custom_views_original.rs). You can copy all of it in a fresh `main.rs` and edit the fresh `Cargo.toml`.

<details>
<summary>Original</summary>

```rust, noplaypen
{{#include ../../../../tests/book/custom_view.rs:original}}
```
</details>

We want to abstract the beginning and end of the system to get this version working.\
The error handling is going to move, we could keep it closer to the original by having a `ResultRenderGraphicsViewMut` for example. 

```rust, noplaypen
{{#include ../../../../tests/book/custom_view.rs:render}}
```

We'll start by creating a struct to hold our init state.

```rust, noplaypen
{{#include ../../../../tests/book/custom_view.rs:custom_view}}
```

Now let's make this struct able to be borrowed and generate the initial state we need.

```rust, noplaypen
{{#include ../../../../tests/book/custom_view.rs:borrow}}
```

We now have a custom view! We can't change our system just yet, we're missing `output`.

Let's add `output` and `graphics` to our custom view.

```rust, noplaypen
{{#include ../../../../tests/book/custom_view.rs:custom_view_full}}
```

Let's revisit our `Borrow` implementation and add one for `Drop`.

```rust, noplaypen
{{#include ../../../../tests/book/custom_view.rs:borrow_revisit}}
```

Our custom view is now fully functional and we successfully moved code that would be duplicated out of the render system.\
You can remove the error handling in `main.rs` to see the result.

As a final touch we can implement `BorrowInfo` to make our view work with workloads.

```rust, noplaypen
{{#include ../../../../tests/book/custom_view.rs:borrow_info}}
```
