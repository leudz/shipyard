# Learn by example

In this section you'll learn how to use shipyard by building a small game.

<iframe src="https://leudz.github.io/shipyard/square_eater" width="645" height="360" title="Square Eater game" scrolling="no"></iframe>

## Dependencies

We'll only use two dependencies, let's add them to `Cargo.toml`.

```toml
macroquad = "0.4.8"
shipyard = { git = "https://github.com/leudz/shipyard", default-features = false, features = [
    "proc",
    "std",
] }
```
