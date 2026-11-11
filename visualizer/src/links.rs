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
use crate::layout::{Rect, WorkloadLayout, BATCH_END_PADDING, BATCH_HEADER_HEIGHT, COLUMN_GAP};
use crate::model::OperationId;
use crate::panels::Point;
use std::cmp::Ordering;
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
///
/// Links sharing a tile, a gutter, a column gap or the band are spread apart, in an order that
/// keeps them from crossing when they can. They still cross when they swap heights, or when one
/// goes across another's whole height.
pub fn routes(layout: &WorkloadLayout, pairs: &BTreeSet<(OperationId, OperationId)>) -> Vec<Route> {
    let tiles = tile_places(layout);
    let links: Vec<Link> = pairs
        .iter()
        .filter_map(|&(blocker, blocked)| {
            let (from, to) = (tiles.get(&blocker)?, tiles.get(&blocked)?);
            // Links only go from a batch to the next one.
            let usable =
                to.batch == from.batch + 1 && is_finite(&from.bounds) && is_finite(&to.bounds);

            usable.then(|| Link {
                blocker,
                blocked,
                from,
                to,
                exits_up: from.column + 1 < layout.batches[from.batch].columns,
                enters_down: to.column > 0,
            })
        })
        .collect();

    // Attach points on shared tiles, in the order the links leave or come into the gutter, so
    // they don't cross there. Ends aren't placed yet, the middle of the other tile stands in.
    let middle = |place: &TilePlace| place.bounds.y + place.bounds.height / 2.0;
    let outgoing = ranks(links.iter().map(|link| {
        let other = middle(link.to);

        (link.blocker, link.leaves(other, other))
    }));
    let incoming = ranks(
        links
            .iter()
            .map(|link| (link.blocked, link.comes_in(middle(link.from)))),
    );
    let ends: Vec<(Point, Point)> = links
        .iter()
        .zip(outgoing.iter().zip(&incoming))
        .map(|(link, (&(out_slot, out_count), &(in_slot, in_count)))| {
            let (from, to) = (link.from.bounds, link.to.bounds);
            let start = Point {
                x: from.right(),
                y: spread(from.y, from.height, out_slot, out_count),
            };
            let end = Point {
                x: to.x,
                y: spread(to.y, to.height, in_slot, in_count),
            };

            (start, end)
        })
        .collect();

    let comes_in: Vec<Height> = links
        .iter()
        .zip(&ends)
        .map(|(link, (start, _))| link.comes_in(start.y))
        .collect();
    // Links reaching the same column through the band leave the gutter in the order they came in.
    let arrivals = ranks(comes_in.iter().map(|&height| ((), height)));
    let leaves: Vec<Height> = links
        .iter()
        .zip(&ends)
        .zip(&arrivals)
        .map(|((link, (_, end)), &(arrival, _))| link.leaves(end.y, arrival as f64))
        .collect();

    // Gutters, from left to right: links going up, then links going down. Going up, the ones
    // coming in lower are further right, they pass in front of the others' entries. Going down,
    // the ones coming in higher are. Rows line up across batches, a link often comes in at the
    // height another leaves at: with this order they don't run along each other there.
    let gutters = ranks(links.iter().zip(&comes_in).zip(&ends).map(
        |((link, &[band, a, b]), (start, end))| {
            let going_up = !link.exits_up && (link.enters_down || end.y < start.y);
            let order = if going_up {
                [0.0, band, a, b]
            } else {
                [1.0, -band, -a, -b]
            };

            (link.from.batch, order)
        },
    ));

    // The band, from top to bottom. Leaving their batch, links from columns further left are
    // higher: they cross the gaps of later columns. Reaching theirs, links to columns further
    // right are higher, for the same reason. Links leaving their batch through the band are above
    // the ones only reaching theirs through it, which climb the gutter under them.
    let bands = ranks(links.iter().zip(comes_in.iter().zip(&leaves)).map(
        |(link, (&[a, b, c], &[d, e, f]))| {
            let order = if link.exits_up {
                [0.0, a, b, c]
            } else {
                [1.0, d, e, f]
            };

            (link.from.batch, order)
        },
    ));
    let lane_ys: Vec<f64> = bands
        .iter()
        .map(|&(rank, count)| spread(LANE_TOP, LANE_BOTTOM - LANE_TOP, rank, count))
        .collect();

    // Column gaps, from left to right: links coming down from the band, the higher ones further
    // right, then links going up to it, the higher ones further left.
    let mut climbs = Vec::new();
    for (index, link) in links.iter().enumerate() {
        if link.enters_down {
            let gap = (link.to.batch, link.to.column - 1);
            climbs.push((index, false, (gap, [0.0, -lane_ys[index]])));
        }
        if link.exits_up {
            let gap = (link.from.batch, link.from.column);
            climbs.push((index, true, (gap, [1.0, lane_ys[index]])));
        }
    }
    let gap_slots = ranks(climbs.iter().map(|&(_, _, entry)| entry));
    let mut entry_xs = vec![0.0; links.len()];
    let mut exit_xs = vec![0.0; links.len()];
    for (&(index, exits, _), &(slot, count)) in climbs.iter().zip(&gap_slots) {
        let link = &links[index];
        if exits {
            exit_xs[index] = spread(link.from.bounds.right(), COLUMN_GAP, slot, count);
        } else {
            entry_xs[index] = spread(link.to.bounds.x - COLUMN_GAP, COLUMN_GAP, slot, count);
        }
    }

    links
        .iter()
        .enumerate()
        .map(|(index, link)| {
            let (gutter_slot, gutter_count) = gutters[index];
            let previous = &layout.batches[link.from.batch];
            let next = &layout.batches[link.to.batch];
            let gutter_left = previous.bounds.right() - BATCH_END_PADDING;
            let xs = Xs {
                exit: exit_xs[index],
                gutter: spread(
                    gutter_left,
                    next.bounds.x - gutter_left,
                    gutter_slot,
                    gutter_count,
                ),
                entry: entry_xs[index],
            };

            Route {
                blocker: link.blocker,
                blocked: link.blocked,
                points: path(ends[index], link, xs, lane_ys[index]),
            }
        })
        .collect()
}

/// A link being routed.
struct Link<'a> {
    blocker: OperationId,
    blocked: OperationId,
    from: &'a TilePlace,
    to: &'a TilePlace,
    /// The blocker isn't in its batch's last column, the link goes up to the band to leave it.
    exits_up: bool,
    /// The blocked tile isn't in its batch's first column, the link comes down from the band.
    enters_down: bool,
}

/// How high a link comes into or leaves a gutter, sorting from highest to lowest. The band is
/// above every tile: `[0.0, ..]` sorts before a tile's `[1.0, y, 0.0]`.
type Height = [f64; 3];

impl Link<'_> {
    /// How high the link comes into the gutter, leaving its blocker at `y`. Through the band,
    /// links from columns further left come in higher.
    fn comes_in(&self, y: f64) -> Height {
        if self.exits_up {
            [0.0, self.from.column as f64, y]
        } else {
            [1.0, y, 0.0]
        }
    }

    /// How high the link leaves the gutter, reaching its blocked tile at `y`. Through the band,
    /// links to columns further right leave higher, `tie` orders the others.
    fn leaves(&self, y: f64, tie: f64) -> Height {
        if self.enters_down {
            [0.0, -(self.to.column as f64), tie]
        } else {
            [1.0, y, 0.0]
        }
    }
}

/// Where a link goes vertically: up its blocker's column gap, down the gutter, and down its
/// blocked tile's column gap. Only the ones its path takes are used.
struct Xs {
    exit: f64,
    gutter: f64,
    entry: f64,
}

/// Numbers each entry within its group, by its key: `(index, entries in the group)`, in the order
/// given. Entries with the same key keep their order.
fn ranks<G: Ord + Copy, const N: usize>(
    entries: impl Iterator<Item = (G, [f64; N])>,
) -> Vec<(usize, usize)> {
    let entries: Vec<(G, [f64; N])> = entries.collect();
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by(|&a, &b| {
        let ((group_a, key_a), (group_b, key_b)) = (&entries[a], &entries[b]);
        let keys = key_a
            .iter()
            .zip(key_b)
            .map(|(a, b)| a.total_cmp(b))
            .find(|ordering| ordering.is_ne())
            .unwrap_or(Ordering::Equal);

        group_a.cmp(group_b).then(keys)
    });

    let mut numbered = vec![(0, 0); entries.len()];
    for group in order.chunk_by(|&a, &b| entries[a].0 == entries[b].0) {
        for (rank, &index) in group.iter().enumerate() {
            numbered[index] = (rank, group.len());
        }
    }

    numbered
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

/// The `slot`-th of `count` evenly spread positions strictly inside `start..start + length`.
fn spread(start: f64, length: f64, slot: usize, count: usize) -> f64 {
    start + length * (slot + 1) as f64 / (count + 1) as f64
}

/// The points from the link's start to its end, see the module's documentation for the spaces it
/// goes through.
fn path((start, end): (Point, Point), link: &Link, xs: Xs, lane_y: f64) -> Vec<Point> {
    let mut points = vec![start];

    // Tiles of later columns are in the way, we go up the column gap and over them.
    if link.exits_up {
        points.push(Point {
            x: xs.exit,
            y: start.y,
        });
        points.push(Point {
            x: xs.exit,
            y: lane_y,
        });
        points.push(Point {
            x: xs.gutter,
            y: lane_y,
        });
    } else {
        points.push(Point {
            x: xs.gutter,
            y: start.y,
        });
    }

    if link.enters_down {
        points.push(Point {
            x: xs.gutter,
            y: lane_y,
        });
        points.push(Point {
            x: xs.entry,
            y: lane_y,
        });
        points.push(Point {
            x: xs.entry,
            y: end.y,
        });
    } else {
        points.push(Point {
            x: xs.gutter,
            y: end.y,
        });
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

    /// How many times a segment of one route crosses a segment of another. Touching and running
    /// along each other don't count.
    fn crossings(routes: &[Route]) -> usize {
        let segments = |route: &Route| -> Vec<(Point, Point)> {
            route
                .points
                .windows(2)
                .map(|segment| (segment[0], segment[1]))
                .collect()
        };
        let cross = |(a, b): (Point, Point), (c, d): (Point, Point)| {
            // A horizontal segment and a vertical one cross strictly inside both.
            let ((h1, h2), (v1, v2)) = if a.y == b.y && c.x == d.x {
                ((a, b), (c, d))
            } else if c.y == d.y && a.x == b.x {
                ((c, d), (a, b))
            } else {
                return false;
            };
            let (left, right) = (h1.x.min(h2.x), h1.x.max(h2.x));
            let (top, bottom) = (v1.y.min(v2.y), v1.y.max(v2.y));

            left < v1.x && v1.x < right && top < h1.y && h1.y < bottom
        };

        let mut count = 0;
        for (index, route) in routes.iter().enumerate() {
            for other in &routes[index + 1..] {
                for segment in segments(route) {
                    for other_segment in segments(other) {
                        count += usize::from(cross(segment, other_segment));
                    }
                }
            }
        }

        count
    }

    /// How many times a segment of one route runs along a segment of another.
    fn overlaps(routes: &[Route]) -> usize {
        let along = |(a, b): (Point, Point), (c, d): (Point, Point)| {
            let shared = |(a1, a2): (f64, f64), (b1, b2): (f64, f64)| {
                a1.min(a2).max(b1.min(b2)) < a1.max(a2).min(b1.max(b2))
            };

            (a.y == b.y && c.y == d.y && a.y == c.y && shared((a.x, b.x), (c.x, d.x)))
                || (a.x == b.x && c.x == d.x && a.x == c.x && shared((a.y, b.y), (c.y, d.y)))
        };

        let mut count = 0;
        for (index, route) in routes.iter().enumerate() {
            for other in &routes[index + 1..] {
                for segment in route.points.windows(2) {
                    for other_segment in other.points.windows(2) {
                        let segment = (segment[0], segment[1]);
                        let other_segment = (other_segment[0], other_segment[1]);
                        count += usize::from(along(segment, other_segment));
                    }
                }
            }
        }

        count
    }

    /// Asserts that links into one tile, from tiles above, beside and below it, don't cross.
    #[test]
    fn fan_in() {
        let layout = layout(&[3, 3]);

        for target in 3..6 {
            let routes = routes(&layout, &pairs(&[(0, target), (1, target), (2, target)]));

            assert_eq!(crossings(&routes), 0, "into {target}: {routes:#?}");
        }
    }

    /// Asserts that links out of one tile, to tiles above, beside and below it, don't cross.
    #[test]
    fn fan_out() {
        let layout = layout(&[3, 3]);

        for source in 0..3 {
            let routes = routes(&layout, &pairs(&[(source, 3), (source, 4), (source, 5)]));

            assert_eq!(crossings(&routes), 0, "out of {source}: {routes:#?}");
        }
    }

    /// Asserts that links between distinct tiles going the same way don't cross.
    ///
    /// Links that swap heights, or one going across another's whole height, can't avoid it.
    #[test]
    fn parallel() {
        let layout = layout(&[3, 3]);

        for pairs_list in [
            [(0, 4), (1, 5)],
            [(1, 3), (2, 4)],
            [(0, 4), (2, 5)],
            [(0, 3), (1, 5)],
            [(1, 4), (2, 5)],
        ] {
            let routes = routes(&layout, &pairs(&pairs_list));

            assert_eq!(crossings(&routes), 0, "{pairs_list:?}: {routes:#?}");
        }
    }

    /// Asserts that links into one tile, or out of one tile, neither cross nor run along each
    /// other, with tiles in several columns.
    #[test]
    fn fans_across_columns() {
        for sizes in [[4, 4], [3, 4], [4, 3], [4, 1], [1, 4], [6, 6]] {
            let layout = layout(&sizes);
            let (first, second) = (0..sizes[0], sizes[0]..sizes[0] + sizes[1]);

            for target in second.clone() {
                let list: Vec<(usize, usize)> =
                    first.clone().map(|source| (source, target)).collect();
                let routes = routes(&layout, &pairs(&list));

                assert_eq!(
                    crossings(&routes),
                    0,
                    "{sizes:?} into {target}: {routes:#?}"
                );
                assert_eq!(overlaps(&routes), 0, "{sizes:?} into {target}: {routes:#?}");
            }
            for source in first.clone() {
                let list: Vec<(usize, usize)> =
                    second.clone().map(|target| (source, target)).collect();
                let routes = routes(&layout, &pairs(&list));

                assert_eq!(
                    crossings(&routes),
                    0,
                    "{sizes:?} out of {source}: {routes:#?}"
                );
                assert_eq!(
                    overlaps(&routes),
                    0,
                    "{sizes:?} out of {source}: {routes:#?}"
                );
            }
        }
    }

    /// Asserts that a link leaving at the height another arrives at doesn't run along it. Rows
    /// line up across batches, so it's common.
    #[test]
    fn same_height() {
        // 0 and 3 are in the first row of batch 0, 4 and 7 in the first row of batch 1.
        let layout = layout(&[4, 4]);

        for list in [[(0, 4), (3, 7)], [(3, 4), (0, 7)], [(0, 5), (3, 4)]] {
            let routes = routes(&layout, &pairs(&list));

            assert_eq!(overlaps(&routes), 0, "{list:?}: {routes:#?}");
        }
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
