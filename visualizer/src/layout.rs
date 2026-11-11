//! Where batches and their tiles go in the workload scene, in unzoomed scene pixels.
//!
//! Systems and regroups both get a tile.
//!
//! Batches run left to right in execution order. Within a batch, tiles fill a column top to
//! bottom and continue in a new column once the available height is used up. The browser decides
//! how tall each tile is, we take the measured heights as input.

use crate::model::{OperationId, Workload};

/// The width of a tile, which is also the width tiles are measured at.
pub const TILE_WIDTH: f64 = 240.0;
/// The space above a batch's first tile, for its header.
pub const BATCH_HEADER_HEIGHT: f64 = 44.0;
/// Used for tiles without a usable measurement.
pub const MIN_TILE_HEIGHT: f64 = 36.0;
/// Columns never get shorter than this, however little room the viewport leaves.
pub const MIN_AVAILABLE_HEIGHT: f64 = 150.0;

const COLUMN_GAP: f64 = 10.0;
const ROW_GAP: f64 = 6.0;
/// Space after a batch's last column, before its separator.
const BATCH_END_PADDING: f64 = 10.0;
/// Space below a batch's tallest column.
const BATCH_BOTTOM_PADDING: f64 = 8.0;
const BATCH_GAP: f64 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkloadLayout {
    /// One per batch of the workload, at the same index.
    pub batches: Vec<BatchLayout>,
    pub width: f64,
    /// The height of the tallest batch.
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchLayout {
    pub bounds: Rect,
    pub columns: usize,
    /// The batch's operations in display order, see [`Batch::operations`](crate::model::Batch::operations).
    pub tiles: Vec<Tile>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pub operation: OperationId,
    pub column: usize,
    /// In scene coordinates, not relative to the batch.
    pub bounds: Rect,
}

/// Lays out `workload`'s batches and tiles.
///
/// `tile_height` gives the measured height of an operation's tile, non-finite or non-positive
/// heights count as [`MIN_TILE_HEIGHT`]. Columns hold at most `available_height` of header and
/// tiles, but at least one tile and never less than [`MIN_AVAILABLE_HEIGHT`].
pub fn layout_workload(
    workload: &Workload,
    tile_height: impl Fn(OperationId) -> f64,
    available_height: f64,
) -> WorkloadLayout {
    let available_height = if available_height.is_finite() {
        available_height.max(MIN_AVAILABLE_HEIGHT)
    } else {
        MIN_AVAILABLE_HEIGHT
    };
    let column_limit = available_height - BATCH_HEADER_HEIGHT;

    let mut batches = Vec::with_capacity(workload.batches.len());
    let mut x = 0.0;
    for batch in &workload.batches {
        let mut tiles = Vec::new();
        // How much of each column the tiles use, including the gap below each tile.
        let mut column_heights = vec![0.0];

        for operation in batch.operations() {
            let height = usable_height(tile_height(operation));

            // A tile that doesn't fit starts the next column, unless its column is still empty:
            // a tile taller than the limit gets a column of its own instead of an endless loop.
            let column_height = *column_heights.last().unwrap();
            if column_height > 0.0 && column_height + height + ROW_GAP > column_limit {
                column_heights.push(0.0);
            }

            let column = column_heights.len() - 1;
            tiles.push(Tile {
                operation,
                column,
                bounds: Rect {
                    x: x + column as f64 * (TILE_WIDTH + COLUMN_GAP),
                    y: BATCH_HEADER_HEIGHT + column_heights[column],
                    width: TILE_WIDTH,
                    height,
                },
            });
            column_heights[column] += height + ROW_GAP;
        }

        let columns = column_heights.len();
        let tallest_column = column_heights.iter().copied().fold(0.0, f64::max);
        let bounds = Rect {
            x,
            y: 0.0,
            width: columns as f64 * (TILE_WIDTH + COLUMN_GAP) + BATCH_END_PADDING,
            height: BATCH_HEADER_HEIGHT + tallest_column + BATCH_BOTTOM_PADDING,
        };

        x = bounds.right() + BATCH_GAP;
        batches.push(BatchLayout {
            bounds,
            columns,
            tiles,
        });
    }

    let width = batches.last().map_or(0.0, |batch| batch.bounds.right());
    let height = batches
        .iter()
        .map(|batch| batch.bounds.height)
        .fold(0.0, f64::max);

    WorkloadLayout {
        batches,
        width,
        height,
    }
}

fn usable_height(measured: f64) -> f64 {
    if measured.is_finite() && measured > 0.0 {
        measured
    } else {
        MIN_TILE_HEIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Batch, OccurrenceId, RegroupId};

    fn system(index: usize) -> OperationId {
        OperationId::System(OccurrenceId(index))
    }

    /// The index of `operation`'s record, for height tables.
    fn index(operation: OperationId) -> usize {
        match operation {
            OperationId::System(occurrence) => occurrence.0,
            OperationId::Regroup(regroup) => regroup.0,
        }
    }

    /// A batch whose occurrences are numbered from `first`, the first one thread-bound if
    /// `thread_bound`.
    fn batch(first: usize, count: usize, thread_bound: bool) -> Batch {
        let mut ids = (first..first + count).map(OccurrenceId);

        Batch {
            thread_bound: if thread_bound { ids.next() } else { None },
            others: ids.collect(),
            regroups: Vec::new(),
        }
    }

    fn workload(batches: Vec<Batch>) -> Workload {
        Workload {
            name: "Workload(test)".to_string(),
            label: "test".to_string(),
            batches,
        }
    }

    fn overlap(a: &Rect, b: &Rect) -> bool {
        a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
    }

    fn contains(outer: &Rect, inner: &Rect) -> bool {
        outer.x <= inner.x
            && inner.right() <= outer.right()
            && outer.y <= inner.y
            && inner.bottom() <= outer.bottom()
    }

    /// Asserts that every tile is finite, inside its batch, and overlaps no other tile, and that
    /// batches don't overlap either.
    fn assert_well_formed(layout: &WorkloadLayout) {
        let tiles: Vec<&Tile> = layout.batches.iter().flat_map(|b| &b.tiles).collect();

        for batch in &layout.batches {
            for tile in &batch.tiles {
                let Rect {
                    x,
                    y,
                    width,
                    height,
                } = tile.bounds;
                assert!([x, y, width, height].iter().all(|v| v.is_finite()));
                assert!(
                    contains(&batch.bounds, &tile.bounds),
                    "{tile:?} outside {batch:?}"
                );
            }
        }
        for (index, a) in tiles.iter().enumerate() {
            for b in &tiles[index + 1..] {
                assert!(!overlap(&a.bounds, &b.bounds), "{a:?} overlaps {b:?}");
            }
        }
        for pair in layout.batches.windows(2) {
            assert!(pair[0].bounds.right() < pair[1].bounds.x);
        }
    }

    /// Asserts that a workload without batches has an empty, finite layout.
    #[test]
    fn empty_workload() {
        let layout = layout_workload(&workload(vec![]), |_| 40.0, 500.0);

        assert!(layout.batches.is_empty());
        assert_eq!((layout.width, layout.height), (0.0, 0.0));
    }

    /// Asserts that empty batches keep their place and index, with one empty column.
    #[test]
    fn empty_batch() {
        let workload = workload(vec![
            batch(0, 1, false),
            batch(1, 0, false),
            batch(1, 1, false),
        ]);

        let layout = layout_workload(&workload, |_| 40.0, 500.0);

        assert_eq!(layout.batches.len(), 3);
        assert!(layout.batches[1].tiles.is_empty());
        assert_eq!(layout.batches[1].columns, 1);
        assert_eq!(layout.batches[2].tiles[0].operation, system(1));
        assert_well_formed(&layout);
    }

    /// Asserts that tiles follow the batch's display order, thread-bound slot first.
    #[test]
    fn order() {
        let workload = workload(vec![Batch {
            thread_bound: Some(OccurrenceId(7)),
            others: vec![OccurrenceId(2), OccurrenceId(5)],
            regroups: Vec::new(),
        }]);

        let layout = layout_workload(&workload, |_| 40.0, 500.0);

        let operations: Vec<OperationId> = layout.batches[0]
            .tiles
            .iter()
            .map(|t| t.operation)
            .collect();
        assert_eq!(operations, [system(7), system(2), system(5)]);
        assert_eq!(layout.batches[0].tiles[0].bounds.y, BATCH_HEADER_HEIGHT);
    }

    /// Asserts that tiles continue in a new column once the available height is used up.
    #[test]
    fn columns() {
        // 44 of header leaves room for three 40 tall tiles with their gaps in 200.
        let layout = layout_workload(&workload(vec![batch(0, 5, false)]), |_| 40.0, 200.0);

        let batch = &layout.batches[0];
        let columns: Vec<usize> = batch.tiles.iter().map(|t| t.column).collect();
        assert_eq!(columns, [0, 0, 0, 1, 1]);
        assert_eq!(batch.tiles[3].bounds.y, BATCH_HEADER_HEIGHT);
        assert_eq!(batch.tiles[3].bounds.x, TILE_WIDTH + COLUMN_GAP);
        assert_eq!(batch.columns, 2);
        assert_well_formed(&layout);
    }

    /// Asserts that measured heights set the tiles' heights and positions.
    #[test]
    fn measured_heights() {
        let heights = [36.0, 59.5, 44.0];
        let layout = layout_workload(
            &workload(vec![batch(0, 3, false)]),
            |o| heights[index(o)],
            500.0,
        );

        let tiles = &layout.batches[0].tiles;
        assert_eq!(tiles[1].bounds.height, 59.5);
        assert_eq!(
            tiles[2].bounds.y,
            BATCH_HEADER_HEIGHT + 36.0 + 59.5 + 2.0 * ROW_GAP
        );
    }

    /// Asserts that a tile taller than the available height gets its own column.
    #[test]
    fn tall_tile() {
        let heights = [40.0, 1000.0, 40.0];
        let layout = layout_workload(
            &workload(vec![batch(0, 3, false)]),
            |o| heights[index(o)],
            200.0,
        );

        let columns: Vec<usize> = layout.batches[0].tiles.iter().map(|t| t.column).collect();
        assert_eq!(columns, [0, 1, 2]);
        assert_well_formed(&layout);
    }

    /// Asserts that unusable measurements and available heights still give finite bounds.
    #[test]
    fn unusable_sizes() {
        let heights = [f64::NAN, -5.0, f64::INFINITY, 0.0];
        let layout = layout_workload(
            &workload(vec![batch(0, 4, false)]),
            |o| heights[index(o)],
            f64::NAN,
        );

        assert!(layout.batches[0]
            .tiles
            .iter()
            .all(|t| t.bounds.height == MIN_TILE_HEIGHT));
        assert_well_formed(&layout);
    }

    /// Asserts that a very small available height still fits a few tiles per column.
    #[test]
    fn small_viewport() {
        let layout = layout_workload(&workload(vec![batch(0, 4, false)]), |_| 36.0, 10.0);

        assert_eq!(layout.batches[0].tiles[1].column, 0);
        assert_well_formed(&layout);
    }

    /// Asserts that more available height, as when zooming out, gives fewer and taller columns.
    #[test]
    fn reflow() {
        let workload = workload(vec![batch(0, 30, false)]);

        let short = layout_workload(&workload, |_| 40.0, 300.0);
        let tall = layout_workload(&workload, |_| 40.0, 900.0);

        assert!(tall.batches[0].columns < short.batches[0].columns);
        assert!(tall.width < short.width);
        assert_well_formed(&short);
        assert_well_formed(&tall);
    }

    /// Asserts that a very wide batch and several varied batches lay out without overlaps.
    #[test]
    fn wide_batches() {
        let workload = workload(vec![
            batch(0, 1000, true),
            batch(1000, 3, false),
            batch(1003, 0, false),
            batch(1003, 200, true),
        ]);
        // Heights between 36 and 96, varying from tile to tile.
        let height = |o: OperationId| 36.0 + (index(o) * 37 % 61) as f64;

        let layout = layout_workload(&workload, height, 700.0);

        assert!(layout.batches[0].columns > 50);
        assert_eq!(layout.batches[3].tiles.len(), 200);
        assert_eq!(layout.width, layout.batches[3].bounds.right());
        assert_well_formed(&layout);
    }

    /// Asserts that two occurrences of the same grouped system get their own tiles.
    #[test]
    fn repeated_occurrences() {
        // Occurrences 0 and 2 stand for the same system, the layout only knows occurrences.
        let workload = workload(vec![batch(0, 2, false), batch(2, 1, false)]);

        let layout = layout_workload(&workload, |_| 40.0, 500.0);

        let first = &layout.batches[0].tiles[0];
        let repeat = &layout.batches[1].tiles[0];
        assert_eq!((first.operation, repeat.operation), (system(0), system(2)));
        assert!(!overlap(&first.bounds, &repeat.bounds));
    }

    /// Asserts that a batch's regroups get tiles after its systems, in the same columns.
    #[test]
    fn mixed_batch() {
        let mut mixed = batch(0, 2, true);
        mixed.regroups = vec![RegroupId(0), RegroupId(1)];

        let layout = layout_workload(&workload(vec![mixed]), |_| 40.0, 200.0);

        let operations: Vec<OperationId> = layout.batches[0]
            .tiles
            .iter()
            .map(|t| t.operation)
            .collect();
        assert_eq!(
            operations,
            [
                system(0),
                system(1),
                OperationId::Regroup(RegroupId(0)),
                OperationId::Regroup(RegroupId(1))
            ]
        );
        assert_eq!(layout.batches[0].columns, 2);
        assert_well_formed(&layout);
    }

    /// Asserts that a batch with only a regroup gets a tile of its measured height.
    #[test]
    fn regroup_only_batch() {
        let mut regrouping = batch(0, 0, false);
        regrouping.regroups = vec![RegroupId(3)];
        let heights = |o: OperationId| match o {
            OperationId::Regroup(_) => 75.0,
            OperationId::System(_) => 40.0,
        };

        let layout = layout_workload(
            &workload(vec![batch(0, 1, false), regrouping]),
            heights,
            500.0,
        );

        let tiles = &layout.batches[1].tiles;
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].operation, OperationId::Regroup(RegroupId(3)));
        assert_eq!(tiles[0].bounds.height, 75.0);
        assert!(layout.batches[1].bounds.height >= BATCH_HEADER_HEIGHT + 75.0);
        assert_well_formed(&layout);
    }
}
