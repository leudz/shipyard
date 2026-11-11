# Shipyard visualizer

Explores the workloads exported by `World::workloads_info`. The user guide's [Visualizer chapter](../guide/master/src/going-further/visualizer.md) describes what it does.

It's a [Dioxus](https://github.com/DioxusLabs/dioxus) application with two renderers, picked by Cargo feature. Enable exactly one:

- `desktop`, the default: a native window with the system WebView.
- `web`: a static Wasm site, the one published at <https://leudz.github.io/shipyard/visualizer/>.

## Building

Builds go through the Dioxus CLI, matching the `dioxus` version in `Cargo.toml`. It bundles `assets/`, a plain `cargo run` starts without the stylesheet.

```sh
cargo install dioxus-cli --version 0.7.10 --locked
```

From this folder:

```sh
# Desktop, with hot reloading.
dx serve --platform desktop

# Browser, with hot reloading. `--platform web` swaps the default `desktop` feature for `web`.
dx serve --platform web

# The site as deployed, in `dist/public`. `Dioxus.toml` serves it beneath `/shipyard/visualizer/`.
dx bundle --platform web --release --out-dir dist

# A release desktop build. Pass a file to open it at launch: `visualizer workloads.json`.
dx build --platform desktop --release
```

On Linux the desktop renderer needs WebKitGTK and a few libraries, for example on Debian and Ubuntu:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev
```

## Checking

```sh
# The model and state tests need no renderer.
cargo test -p visualizer --lib --no-default-features

cargo clippy -p visualizer --all-targets
cargo clippy -p visualizer --target wasm32-unknown-unknown --no-default-features --features web
```

Don't use `--all-features`, it enables both renderers.

`src/conflict_workloads.json` is a real export, it checks that the visualizer reads what shipyard writes. After an intended change to the exported information, regenerate it:

```sh
cargo test -p visualizer --lib --no-default-features -- --ignored regenerate
```
