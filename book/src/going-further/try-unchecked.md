# Might want to try_ or go _unchecked

These two are a prefix and a suffix respectively.

### Unchecked

Some functions come with an `_unchecked` version. They might be memory unsafe and marked as such or "just" logically unsafe, in this case using it in the wrong conditions will result in unexpected behavior but won't cause Undefined Behavior.

Currently the only function of this kind is [`Entities::delete_unchecked`](https://docs.rs/shipyard/latest/shipyard/struct.Entities.html#method.delete_unchecked) but others will likely join it, like [`add_component_unchecked`](https://github.com/leudz/shipyard/issues/50).

### Try

All functions provided by shipyard should either not fail in any circumstances or have a `try_` alternative. If it isn't the case, please open an issue or PR.
