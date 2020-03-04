# Getting Started

Just add it to Cargo.toml 😄

For WASM or single-threaded environments you'll need to turn off default features and remember to add other features back in, like this:

```toml
shipyard = { version = "0.3.1", features = ["proc", "serde"], default-features = false }
```
