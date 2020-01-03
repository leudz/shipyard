# Syntactic Weirdness

## Tuples

## Double `mut`
So what's with the double `mut` in the system definitions (e.g. **mut** empties:&**mut** Empty)? Let's think about what we need the system to change. In this case it's for sure `entities` since we're adding a new entity, but there's more - it's actually the components too. More precisely, it's not that we're mutating the _contents_ of a specific component in this example - it's that we're adding a new component to the underlying `Storage` that contains the components. We need a mutable reference to the storage. In Shipyard terms - this means we need the system to get a `ViewMut`, e.g. a mutable _view_ into the `Storage`.

With that in mind there's actually two things we need to inform the compiler so that it can do its magic:

1. We need to tell Rust that we want a mutable reference to the variables. This is the first `mut`
2. We need to tell Shipyard that we want a `ViewMut` (as opposed to a `View`). This is the second `mut`

## `ref mut`
