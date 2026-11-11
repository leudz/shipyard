//! Where conflict links go: their paths between tiles, and the cues standing in for tiles off screen.
//!
//! Paths are in scene coordinates, like [`crate::layout`]. They only use space the layout already
//! leaves free, so showing or hiding links never moves a tile:
//! - the column gaps, to leave or reach a tile that isn't next to the batch's edge,
//! - the band between the batch headers and the first row of tiles, to cross columns,
//! - the gutter between two batches, to go from one batch to the next.
//!
//! Nothing here changes the camera, cues only point to what's off screen.

use crate::camera::Camera;
use crate::layout::{
    BatchLayout, Rect, WorkloadLayout, BATCH_END_PADDING, BATCH_HEADER_HEIGHT, COLUMN_GAP,
};
use crate::model::OperationId;
use crate::panels::Point;
use std::collections::{BTreeMap, BTreeSet};

/// Where the band crossing columns starts, below the batch header's text.
const LANE_TOP: f64 = BATCH_HEADER_HEIGHT - 10.0;
/// Where the band crossing columns ends, just above the first row of tiles.
const LANE_BOTTOM: f64 = BATCH_HEADER_HEIGHT - 2.0;
/// The minimum distance between two cues on the same edge of the screen, in screen pixels.
pub const CUE_SPACING: f64 = 22.0;

/// A link's path from its blocker's right edge to its blocked tile's left edge.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub blocker: OperationId,
    pub blocked: OperationId,
    /// In scene coordinates. Every segment is horizontal or vertical, the last one points right.
    pub points: Vec<Point>,
}

/// Which edge of the screen a cue sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// Stands in for a link whose other end is off screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cue {
    pub blocker: OperationId,
    pub blocked: OperationId,
    /// The end that's off screen, the cue names it.
    pub hidden: OperationId,
    pub side: Side,
    /// In screen pixels, lined up with the link's visible end when other cues leave room.
    pub y: f64,
}

/// How the scene is drawn on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub camera: Camera,
    /// The scene's width on screen.
    pub width: f64,
    /// Where scene y 0 is on screen.
    pub top: f64,
}

impl View {
    pub fn to_screen(self, point: Point) -> Point {
        Point {
            x: self.camera.to_screen(point.x),
            y: self.top + point.y * self.camera.zoom,
        }
    }
}

/// Routes every `(blocker, blocked)` pair of `pairs` through `layout`.
///
/// Pairs without both tiles in the layout, or with unusable bounds, get no route.
pub fn routes(layout: &WorkloadLayout, pairs: &BTreeSet<(OperationId, OperationId)>) -> Vec<Route> {
    let tiles = tile_places(layout);
    let usable: Vec<(OperationId, OperationId)> = pairs
        .iter()
        .copied()
        .filter(|(blocker, blocked)| {
            let (Some(from), Some(to)) = (tiles.get(blocker), tiles.get(blocked)) else {
                return false;
            };

            // Links only go from a batch to the next one.
            to.batch == from.batch + 1 && is_finite(&from.bounds) && is_finite(&to.bounds)
        })
        .collect();

    // Each link gets its own attach point on shared tiles, and its own lane in a shared gutter.
    let outgoing = slots(usable.iter().map(|&(blocker, blocked)| (blocker, blocked)));
    let incoming = slots(usable.iter().map(|&(blocker, blocked)| (blocked, blocker)));
    let gutters = slots(
        usable
            .iter()
            .map(|&(blocker, blocked)| (tiles[&blocker].batch, (blocker, blocked))),
    );

    usable
        .iter()
        .map(|&(blocker, blocked)| {
            let from = &tiles[&blocker];
            let to = &tiles[&blocked];
            let (out_slot, out_count) = outgoing[&(blocker, blocked)];
            let (in_slot, in_count) = incoming[&(blocked, blocker)];
            let (gutter_slot, gutter_count) = gutters[&(from.batch, (blocker, blocked))];

            let start = Point {
                x: from.bounds.right(),
                y: spread(from.bounds.y, from.bounds.height, out_slot, out_count),
            };
            let end = Point {
                x: to.bounds.x,
                y: spread(to.bounds.y, to.bounds.height, in_slot, in_count),
            };
            let previous = &layout.batches[from.batch];
            let next = &layout.batches[to.batch];
            let gutter_left = previous.bounds.right() - BATCH_END_PADDING;
            let gutter_x = spread(
                gutter_left,
                next.bounds.x - gutter_left,
                gutter_slot,
                gutter_count,
            );
            let lane_y = spread(LANE_TOP, LANE_BOTTOM - LANE_TOP, gutter_slot, gutter_count);

            Route {
                blocker,
                blocked,
                points: path(start, end, from, previous, to, gutter_x, lane_y),
            }
        })
        .collect()
}

/// The cues of `routes` with one end off screen, stacked so cues on the same edge don't overlap.
///
/// Links with both ends off screen get none, nothing on screen would lead to them.
pub fn cues(routes: &[Route], layout: &WorkloadLayout, view: View) -> Vec<Cue> {
    let tiles = tile_places(layout);
    let mut cues = Vec::new();

    for route in routes {
        let (Some(from), Some(to)) = (tiles.get(&route.blocker), tiles.get(&route.blocked)) else {
            continue;
        };
        let (Some(&start), Some(&end)) = (route.points.first(), route.points.last()) else {
            continue;
        };
        let shows = |bounds: &Rect| view.camera.shows(bounds.x, bounds.right(), view.width);

        let (hidden, hidden_bounds, visible_end) = match (shows(&from.bounds), shows(&to.bounds)) {
            (false, true) => (route.blocker, from.bounds, end),
            (true, false) => (route.blocked, to.bounds, start),
            (true, true) | (false, false) => continue,
        };
        let side = if view.camera.to_screen(hidden_bounds.right()) <= 0.0 {
            Side::Left
        } else {
            Side::Right
        };

        cues.push(Cue {
            blocker: route.blocker,
            blocked: route.blocked,
            hidden,
            side,
            y: view.to_screen(visible_end).y,
        });
    }

    stack(&mut cues, Side::Left);
    stack(&mut cues, Side::Right);

    cues
}

/// Where an operation's tile is.
struct TilePlace {
    batch: usize,
    column: usize,
    bounds: Rect,
}

fn tile_places(layout: &WorkloadLayout) -> BTreeMap<OperationId, TilePlace> {
    let mut places = BTreeMap::new();

    for (batch, batch_layout) in layout.batches.iter().enumerate() {
        for tile in &batch_layout.tiles {
            places.insert(
                tile.operation,
                TilePlace {
                    batch,
                    column: tile.column,
                    bounds: tile.bounds,
                },
            );
        }
    }

    places
}

fn is_finite(bounds: &Rect) -> bool {
    [bounds.x, bounds.y, bounds.width, bounds.height]
        .iter()
        .all(|value| value.is_finite())
}

/// Numbers each `(group, item)` within its group: `(index, group size)`, in sorted order.
fn slots<G: Ord + Copy, I: Ord + Copy>(
    entries: impl Iterator<Item = (G, I)>,
) -> BTreeMap<(G, I), (usize, usize)> {
    let mut groups: BTreeMap<G, Vec<I>> = BTreeMap::new();
    for (group, item) in entries {
        groups.entry(group).or_default().push(item);
    }

    let mut numbered = BTreeMap::new();
    for (group, mut items) in groups {
        items.sort();
        let count = items.len();

        for (index, item) in items.into_iter().enumerate() {
            numbered.insert((group, item), (index, count));
        }
    }

    numbered
}

/// The `slot`-th of `count` evenly spread positions strictly inside `start..start + length`.
fn spread(start: f64, length: f64, slot: usize, count: usize) -> f64 {
    start + length * (slot + 1) as f64 / (count + 1) as f64
}

/// The points from `start` to `end`, see the module's documentation for the spaces it goes through.
fn path(
    start: Point,
    end: Point,
    from: &TilePlace,
    previous: &BatchLayout,
    to: &TilePlace,
    gutter_x: f64,
    lane_y: f64,
) -> Vec<Point> {
    let mut points = vec![start];

    // Tiles of later columns are in the way, we go up the column gap and over them.
    let last_column = previous.columns - 1;
    if from.column == last_column {
        points.push(Point {
            x: gutter_x,
            y: start.y,
        });
    } else {
        let gap_x = start.x + COLUMN_GAP / 2.0;
        points.push(Point {
            x: gap_x,
            y: start.y,
        });
        points.push(Point {
            x: gap_x,
            y: lane_y,
        });
        points.push(Point {
            x: gutter_x,
            y: lane_y,
        });
    }

    if to.column == 0 {
        points.push(Point {
            x: gutter_x,
            y: end.y,
        });
    } else {
        let gap_x = end.x - COLUMN_GAP / 2.0;
        points.push(Point {
            x: gutter_x,
            y: lane_y,
        });
        points.push(Point {
            x: gap_x,
            y: lane_y,
        });
        points.push(Point { x: gap_x, y: end.y });
    }
    points.push(end);

    points.dedup();

    points
}

/// Moves `side`'s cues down until they're at least [`CUE_SPACING`] apart, keeping their order.
fn stack(cues: &mut [Cue], side: Side) {
    let mut on_side: Vec<&mut Cue> = cues.iter_mut().filter(|cue| cue.side == side).collect();
    on_side.sort_by(|a, b| {
        a.y.total_cmp(&b.y)
            .then((a.blocker, a.blocked).cmp(&(b.blocker, b.blocked)))
    });

    let mut lowest = f64::NEG_INFINITY;
    for cue in on_side {
        cue.y = cue.y.max(lowest + CUE_SPACING);
        lowest = cue.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{layout_workload, TILE_WIDTH};
    use crate::model::{Batch, OccurrenceId, Workload};

    fn sys(index: usize) -> OperationId {
        OperationId::System(OccurrenceId(index))
    }

    /// A batch of systems numbered from `first`.
    fn batch(first: usize, count: usize) -> Batch {
        Batch {
            thread_bound: None,
            others: (first..first + count).map(OccurrenceId).collect(),
            regroups: Vec::new(),
        }
    }

    /// Lays out batches of `sizes` systems, numbered in order, with columns of three 40 tall tiles.
    fn layout(sizes: &[usize]) -> WorkloadLayout {
        let mut first = 0;
        let batches = sizes
            .iter()
            .map(|&size| {
                let batch = batch(first, size);
                first += size;
                batch
            })
            .collect();
        let workload = Workload {
            name: "w".to_string(),
            label: "w".to_string(),
            batches,
        };

        layout_workload(&workload, |_| 40.0, 200.0)
    }

    fn pairs(pairs: &[(usize, usize)]) -> BTreeSet<(OperationId, OperationId)> {
        pairs.iter().map(|&(a, b)| (sys(a), sys(b))).collect()
    }

    fn bounds(layout: &WorkloadLayout, operation: OperationId) -> Rect {
        layout
            .batches
            .iter()
            .flat_map(|batch| &batch.tiles)
            .find(|tile| tile.operation == operation)
            .unwrap()
            .bounds
    }

    /// Asserts that no segment of `route` goes through a tile, edges excepted.
    fn assert_clear(layout: &WorkloadLayout, route: &Route) {
        for segment in route.points.windows(2) {
            let (a, b) = (segment[0], segment[1]);
            let (left, right) = (a.x.min(b.x), a.x.max(b.x));
            let (top, bottom) = (a.y.min(b.y), a.y.max(b.y));

            for tile in layout.batches.iter().flat_map(|batch| &batch.tiles) {
                let r = tile.bounds;
                let crosses = left < r.right() && r.x < right && top < r.bottom() && r.y < bottom;
                let inside_point = left == right
                    && top == bottom
                    && r.x < left
                    && left < r.right()
                    && r.y < top
                    && top < r.bottom();
                assert!(!crosses && !inside_point, "{segment:?} crosses {tile:?}");
            }
        }
    }

    /// Asserts that every segment is horizontal or vertical and the last one points right.
    fn assert_orthogonal(route: &Route) {
        for segment in route.points.windows(2) {
            assert!(segment[0].x == segment[1].x || segment[0].y == segment[1].y);
        }
        let [.., before, last] = route.points[..] else {
            panic!("a route has at least two points");
        };
        assert!(before.x < last.x && before.y == last.y);
    }

    fn view(x: f64, zoom: f64, width: f64) -> View {
        View {
            camera: Camera { x, zoom },
            width,
            top: 100.0,
        }
    }

    /// Asserts that a link between single-column batches goes from the blocker's right edge to the
    /// blocked tile's left edge, through the gutter.
    #[test]
    fn adjacent() {
        let layout = layout(&[1, 1]);

        let routes = routes(&layout, &pairs(&[(0, 1)]));

        let route = &routes[0];
        let (from, to) = (bounds(&layout, sys(0)), bounds(&layout, sys(1)));
        assert_eq!(route.points[0].x, from.right());
        assert_eq!(route.points.last().unwrap().x, to.x);
        assert_orthogonal(route);
        assert_clear(&layout, route);
    }

    /// Asserts that a blocker in a batch's first column goes around the later columns.
    #[test]
    fn blocker_first_column() {
        // Batch 0 has two columns: 0, 1, 2 then 3.
        let layout = layout(&[4, 1]);
        assert_eq!(layout.batches[0].columns, 2);

        let routes = routes(&layout, &pairs(&[(0, 4)]));

        assert_orthogonal(&routes[0]);
        assert_clear(&layout, &routes[0]);
    }

    /// Asserts that a blocked tile in a later column is reached without crossing the first column.
    #[test]
    fn blocked_later_column() {
        // Batch 1 has two columns: 1, 2, 3 then 4.
        let layout = layout(&[1, 4]);

        let routes = routes(&layout, &pairs(&[(0, 4)]));

        let route = &routes[0];
        assert_eq!(route.points.last().unwrap().x, bounds(&layout, sys(4)).x);
        assert_orthogonal(route);
        assert_clear(&layout, route);
    }

    /// Asserts that links sharing a blocker leave it at different heights, and links sharing a
    /// gutter go down it at different places.
    #[test]
    fn shared_endpoint() {
        let layout = layout(&[1, 2]);

        let routes = routes(&layout, &pairs(&[(0, 1), (0, 2)]));

        assert_ne!(routes[0].points[0].y, routes[1].points[0].y);
        assert_ne!(routes[0].points[1].x, routes[1].points[1].x);
        for route in &routes {
            assert_clear(&layout, route);
        }
    }

    /// Asserts that two links into the same tile arrive at different heights.
    #[test]
    fn shared_blocked() {
        let layout = layout(&[2, 1]);

        let routes = routes(&layout, &pairs(&[(0, 2), (1, 2)]));

        let arrivals: Vec<f64> = routes
            .iter()
            .map(|route| route.points.last().unwrap().y)
            .collect();
        assert_ne!(arrivals[0], arrivals[1]);
    }

    /// Asserts that a pair without a tile, or not in consecutive batches, gets no route.
    #[test]
    fn unroutable() {
        let layout = layout(&[1, 1, 1]);

        let routes = routes(&layout, &pairs(&[(0, 2), (0, 9), (1, 0)]));

        assert!(routes.is_empty());
    }

    /// Asserts that unusable tile bounds give no route rather than a NaN path.
    #[test]
    fn invalid_bounds() {
        let mut layout = layout(&[1, 1]);
        layout.batches[1].tiles[0].bounds.y = f64::NAN;

        let routes = routes(&layout, &pairs(&[(0, 1)]));

        assert!(routes.is_empty());
    }

    /// Asserts that the screen transform follows the camera's pan and zoom.
    #[test]
    fn transform() {
        let point = Point { x: 200.0, y: 50.0 };

        let screen = view(-100.0, 0.5, 800.0).to_screen(point);

        assert_eq!(screen, Point { x: 0.0, y: 125.0 });
    }

    /// Asserts that a link with both ends on screen, even partly, gets no cue.
    #[test]
    fn partly_visible() {
        let layout = layout(&[1, 1]);
        let routes = routes(&layout, &pairs(&[(0, 1)]));
        // The blocker's tile is half off the left edge.
        let camera_x = -TILE_WIDTH / 2.0;

        let cues = cues(&routes, &layout, view(camera_x, 1.0, 2000.0));

        assert!(cues.is_empty());
    }

    /// Asserts that a blocker off the left edge gets a left cue naming it, at the blocked tile's end.
    #[test]
    fn hidden_blocker() {
        let layout = layout(&[1, 1]);
        let routes = routes(&layout, &pairs(&[(0, 1)]));
        let blocked = bounds(&layout, sys(1));
        let view = view(-blocked.x + 5.0, 1.0, 1000.0);

        let cues = cues(&routes, &layout, view);

        let [cue] = cues[..] else {
            panic!("expected one cue");
        };
        assert_eq!((cue.hidden, cue.side), (sys(0), Side::Left));
        assert_eq!(cue.y, view.to_screen(*routes[0].points.last().unwrap()).y);
    }

    /// Asserts that a blocked tile off the right edge gets a right cue naming it.
    #[test]
    fn hidden_blocked() {
        let layout = layout(&[1, 1]);
        let routes = routes(&layout, &pairs(&[(0, 1)]));
        let width = bounds(&layout, sys(1)).x - 10.0;

        let cues = cues(&routes, &layout, view(0.0, 1.0, width));

        assert_eq!(cues.len(), 1);
        assert_eq!((cues[0].hidden, cues[0].side), (sys(1), Side::Right));
    }

    /// Asserts that links with both ends off screen get no cue.
    #[test]
    fn both_hidden() {
        let layout = layout(&[1, 1]);
        let routes = routes(&layout, &pairs(&[(0, 1)]));

        let cues = cues(&routes, &layout, view(-10_000.0, 1.0, 500.0));

        assert!(cues.is_empty());
    }

    /// Asserts that cues on the same edge are pushed apart so each can be targeted.
    #[test]
    fn stacked_cues() {
        let layout = layout(&[3, 1]);
        let routes = routes(&layout, &pairs(&[(0, 3), (1, 3), (2, 3)]));
        let blocked = bounds(&layout, sys(3));

        let cues = cues(&routes, &layout, view(-blocked.x + 5.0, 1.0, 400.0));

        assert_eq!(cues.len(), 3);
        let mut ys: Vec<f64> = cues.iter().map(|cue| cue.y).collect();
        ys.sort_by(f64::total_cmp);
        assert!(ys.windows(2).all(|pair| pair[1] - pair[0] >= CUE_SPACING));
    }

    /// Asserts that computing routes and cues leaves the camera as it was.
    #[test]
    fn camera_untouched() {
        let layout = layout(&[1, 1]);
        let view = view(-150.0, 0.8, 300.0);

        let routes = routes(&layout, &pairs(&[(0, 1)]));
        let _ = cues(&routes, &layout, view);

        assert_eq!(
            view.camera,
            Camera {
                x: -150.0,
                zoom: 0.8
            }
        );
    }
}
