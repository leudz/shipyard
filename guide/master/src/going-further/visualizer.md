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

Workloads using [grouped storages](./grouping.md) also run regroups, which the workload adds on its own between your systems.
A regroup has a tile in its batch like a system, with a dashed outline, the word **Regroup** and the storages it regroups.
Two regroups of the same storages in different batches are two tiles.

- Scroll or drag the empty space to pan, <kbd>Ctrl</kbd> + scroll to zoom.
- The buttons at the bottom zoom in and out, reset to 100%, and **Fit** the whole workload on screen.
- The strip at the bottom shows the whole workload, one bar per batch. Taller bars have more systems, the count is written on bars wide enough for it, hover or focus a bar to see its systems. Click a bar to go to its batch, or use the slider below it.

## Inspectors

Clicking a system or component opens an inspector.

- A system's inspector lists the storages it borrows, exclusively or shared, and every batch it appears in. Click an appearance to go to it.
- A component's inspector lists the systems borrowing it, exclusively or shared.
- A regroup's inspector shows where it runs, the storages it borrows, always exclusively, and why the workload placed it:
  an **entry** regroup prepares its storages for changes made outside the workload, a **pending** regroup follows systems borrowing its storages exclusively.
  Each regroup has its own inspector, even when another regroup has the same storages.

Clicking empty space or pressing <kbd>Escape</kbd> in the workload deselects the tile, inspectors and the highlight stay.

### Global map

**Global map**, next to the search fields, lists every component and system of the export.\
Each row highlights its record, the magnifying glass opens it in an inspector.

## Highlighting

**Highlight**, in an inspector or the global map, marks how everything relates to a system or component:

- For a component, systems borrowing it exclusively are red and marked **EX**, systems borrowing it shared are blue and marked **SH**. Regroups of the component are exclusive borrowers.
- For a system, its tiles are outlined, and the storages it borrows are marked in the global map.
- For a regroup, its tile is outlined and its storages are marked.

Unrelated tiles are dimmed. The highlight stays when you switch workloads, the bottom panel says what's highlighted and clears it.

This helps finding what prevents parallelism.\
For example a system borrowing `AllStorages` runs alone, maybe it could borrow individual storages instead.

## Why isn't it in the previous batch?

The selected tile, and every tile the highlight marks, show arrows from the tiles of the previous batch that kept them out of it.
An arrow goes from the tile running first to the one that had to wait. There is one arrow per pair, whatever the number of reasons.

- Only the batch right before is considered. Something further back doesn't get an arrow, even if it has to run earlier.
- When a regroup has to run first, its own arrows from the batch before it show too, they explain where the regroup is.
- A tile in its workload's first batch has no arrow.

Hover or focus an arrow to see why, the pointer can move onto the explanation to scroll it, the arrow keys scroll it too:

- **Borrows**: the storages both borrow in a way they can't share, how each borrows them, and whether it's a condition's borrow.
- **Ordering**: the workload's ordering, like `before_all`, `after_all`, barriers, sequential workloads, or a conflict with a system added earlier.
- **Thread**: both have to run on the thread owning the `World`, a batch only has one such slot.

<kbd>Escape</kbd> hides the explanation and deselects the tile.\
Arrows never move the view. When one end is off screen, a label at the edge of the screen names it, pan to see it.

The explanations come from the export: what the scheduler declared, not what happened at runtime.
A system whose condition didn't pass still has its arrows, and a pending regroup doesn't mean anything changed.

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
