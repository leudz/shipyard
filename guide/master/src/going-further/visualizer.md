# Visualizer

The visualizer shows how your workloads are scheduled: which systems run in parallel, in what order batches run, and which storages each system borrows.

It runs in the browser at the [visualizer page](https://leudz.github.io/shipyard/visualizer/), or as a desktop application built from source.

## Exporting your workloads

The visualizer reads a `.json` file containing the workloads you want to inspect.\
You'll need the `serde1` feature on `shipyard` and `serde_json` as a dependency.\
Then you can run this snippet after the workloads have been added to the `World`.

```rust
// Add your workloads to the World anywhere before

std::fs::write(
    "workloads.json",
    serde_json::to_string(&world.workloads_info()).unwrap(),
)
.unwrap();
```

## Opening a file

Use **Open file…** or drop the file anywhere on the page.\
To follow along with the `square_eater` example, click **Load example**.

The visualizer shows one export at a time.\
Nothing is stored: closing the page or the application forgets the file and the layout.

## The workload

The selected workload's batches are laid out left to right, in execution order.\
A batch is a list of systems that will attempt to run in parallel, there is no ordering within a batch: systems are sorted by name only to be easy to find.\
The system marked with ◆ runs in the batch's thread-bound slot, on the thread owning the `World`.

The same system can appear in several batches or workloads, each appearance is its own tile.

- Scroll or drag the empty space to pan, <kbd>Ctrl</kbd> + scroll to zoom.
- The buttons at the bottom zoom in and out, reset to 100%, and **Fit** the whole workload on screen.
- The strip at the bottom shows the whole workload, one bar per batch. Taller bars have more systems, the count is written on bars wide enough for it, hover or focus a bar to see its systems. Click a bar to go to its batch, or use the slider below it.

## Inspectors

Clicking a system or component opens an inspector.

- A system's inspector lists the storages it borrows, exclusively or shared, and every batch it appears in. Click an appearance to go to it.
- A component's inspector lists the systems borrowing it, exclusively or shared.

### Global map

**Global map**, next to the search fields, lists every component and system of the export.\
Each row highlights its record, the magnifying glass opens it in an inspector.

## Highlighting

**Highlight**, in an inspector or the global map, marks how everything relates to a system or component:

- For a component, systems borrowing it exclusively are red and marked **EX**, systems borrowing it shared are blue and marked **SH**.
- For a system, its tiles are outlined, and the storages it borrows are marked in the global map.

Unrelated tiles are dimmed. The highlight stays when you switch workloads, the bottom panel says what's highlighted and clears it.

This helps finding what prevents parallelism.\
For example a system borrowing `AllStorages` runs alone, maybe it could borrow individual storages instead.

## Desktop application

The desktop application is the same interface in a native window.\
It can also follow the file: **Auto-reload file**, next to the file name, reloads it whenever it's saved.\
Reloading resets the view like opening a file, without asking. A save that isn't a valid export shows an error and keeps the current one until the next valid save.

The desktop application is built from the repository with the [Dioxus CLI](https://github.com/DioxusLabs/dioxus), in the version matching `visualizer/Cargo.toml`.

```sh
cargo install dioxus-cli --version 0.7.10 --locked
cd visualizer
dx build --platform desktop --release
```

`dx` prints where it put the application. Give it a file to open it at launch, for example `visualizer workloads.json`.

It uses the system's WebView: WebView2 on Windows, which Windows 10 and 11 include, WebKit on macOS, and WebKitGTK on Linux, where you'll need its development packages, for example on Debian and Ubuntu:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev
```
