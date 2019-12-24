# Getting Started

Just add it to Cargo.toml :)

For WASM or single-threaded environments you'll need to turn off default features, and remember to add other features back in, like this:

```
shipyard = { features = ["proc", "serialization"], default-features = false, git = "https://github.com/leudz/shipyard.git" }
```