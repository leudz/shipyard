//! Where the bottom panel's strip draws each batch: the whole workload in execution order, seen
//! through a magnifying lens that follows the camera.
//!
//! Batches near the middle of the screen get wide spans, the others share what's left, narrower
//! the further they are. A span's width says how close its batch is to the view, not how large the
//! batch is: the bar's height already shows its system count, and sizing spans by content would
//! let a few large batches squeeze everything else out of the strip.

use crate::camera::Camera;
use crate::layout::WorkloadLayout;

/// The share of the strip split evenly between all batches, so distant batches keep some width.
const UNIFORM_SHARE: f64 = 0.45;
/// The share of the strip the lens hands out to batches near its center.
const LENS_SHARE: f64 = 0.55;
/// How far the lens reaches, in batches: the standard deviation of its Gaussian.
const LENS_SPREAD: f64 = 4.2;
/// The area under the lens' Gaussian, `LENS_SPREAD * sqrt(2π)` rounded. Dividing by it makes a
/// lens away from the workload's ends hand out about [`LENS_SHARE`] of the strip.
const LENS_AREA: f64 = 10.528;

/// A horizontal part of the strip, in pixels from its left edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub left: f64,
    pub right: f64,
}

impl Span {
    pub fn width(self) -> f64 {
        self.right - self.left
    }
}

/// The strip's geometry for one camera, see [`lens`].
#[derive(Debug, Clone, PartialEq)]
pub struct Lens {
    /// One span per batch, at the same index. They're adjacent and cover the whole strip.
    pub batches: Vec<Span>,
    /// The part of the strip matching the screen, `None` when no batch is on screen.
    pub viewport: Option<Span>,
}

/// Lays out a strip `strip_width` wide for the workload `layout`, seen by `camera` on a scene
/// `scene_width` wide.
///
/// The lens looks at a point of the screen that sweeps it as the workload scrolls by, like a
/// scrollbar's thumb: the screen's left edge at the workload's start, its middle halfway, its
/// right edge at the workload's end. So the first and last batches get the full lens at either
/// end. When the whole workload is on screen, the lens is on its middle. When the screen is past
/// an end of the workload, it stays on that end.
///
/// Returns `None` when there's nothing to draw: the workload has no batches, or a width or the
/// camera isn't usable, like a strip that isn't measured yet.
pub fn lens(
    layout: &WorkloadLayout,
    camera: Camera,
    scene_width: f64,
    strip_width: f64,
) -> Option<Lens> {
    let usable = |value: f64| value.is_finite() && value > 0.0;
    if layout.batches.is_empty()
        || !usable(layout.width)
        || !usable(scene_width)
        || !usable(strip_width)
        || !usable(camera.zoom)
        || !camera.x.is_finite()
    {
        return None;
    }

    // The screen's edges, in workload coordinates. Clamping keeps the lens on the workload's
    // nearest end when the screen is past it.
    let to_workload =
        |screen_x: f64| ((screen_x - camera.x) / camera.zoom).clamp(0.0, layout.width);
    let screen_left = to_workload(0.0);
    let screen_right = to_workload(scene_width);

    // How far the workload has scrolled by: the share of its off screen part that's on the left.
    // With nothing off screen, we settle on the middle.
    let hidden_left = screen_left;
    let hidden = hidden_left + (layout.width - screen_right);
    let scrolled = if hidden > 0.0 {
        hidden_left / hidden
    } else {
        0.5
    };
    let focus = screen_left + (screen_right - screen_left) * scrolled;
    // Batch `i`'s middle is at `i + 0.5`. Going no further than the first and last batches'
    // middles gives them the lens' full size at the workload's ends.
    let count = layout.batches.len();
    let center = batch_position(layout, focus).clamp(0.5, count as f64 - 0.5);

    // Running totals of the batches' weights: batch `i` goes from the `i`th to the `i + 1`th.
    // Spans sharing their edges makes them adjacent, and dividing by the last total makes the
    // last span end exactly at the strip's right edge, however close to the ends the lens is.
    let mut edges = Vec::with_capacity(count + 1);
    let mut total = 0.0;
    edges.push(total);
    for index in 0..count {
        // How far the batch's middle is from the lens' center, in spreads.
        let distance = (index as f64 + 0.5 - center) / LENS_SPREAD;
        let weight = UNIFORM_SHARE / count as f64
            + LENS_SHARE / LENS_AREA * (-0.5 * distance * distance).exp();
        total += weight;
        edges.push(total);
    }
    let batches: Vec<Span> = edges
        .windows(2)
        .map(|pair| Span {
            left: pair[0] / total * strip_width,
            right: pair[1] / total * strip_width,
        })
        .collect();

    // The screen's edges go through the same spans as the lens' center, so the viewport lines up
    // with the bars under it.
    let on_screen = layout
        .batches
        .iter()
        .any(|batch| camera.shows(batch.bounds.x, batch.bounds.right(), scene_width));
    let viewport = on_screen.then(|| Span {
        left: project(layout, &batches, screen_left),
        right: project(layout, &batches, screen_right),
    });

    Some(Lens { batches, viewport })
}

/// Where workload coordinate `x` is, counted in batches: `i` at batch `i`'s start, going linearly
/// to `i + 1` at the next batch's start. The last batch reaches `len` at the workload's end.
///
/// We need the real batch starts: batches have different widths, and the gap after a batch
/// counts as part of it so the position doesn't jump or stall between batches.
fn batch_position(layout: &WorkloadLayout, x: f64) -> f64 {
    let batches = &layout.batches;

    // The last batch starting at or before `x`. The first batch starts at 0 and `x` isn't
    // negative, so there's always one.
    let index = batches
        .partition_point(|batch| batch.bounds.x <= x)
        .saturating_sub(1);
    let start = batches[index].bounds.x;
    let end = batches
        .get(index + 1)
        .map_or(layout.width, |next| next.bounds.x);

    let length = end - start;
    let fraction = if length > 0.0 {
        ((x - start) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };

    index as f64 + fraction
}

/// Where workload coordinate `x` lands in the strip, `batches` being the batches' spans.
fn project(layout: &WorkloadLayout, batches: &[Span], x: f64) -> f64 {
    let position = batch_position(layout, x);

    // The workload's end is at `len`, which is the last span's right edge.
    let index = (position as usize).min(batches.len() - 1);
    let span = batches[index];

    span.left + span.width() * (position - index as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{MAX_ZOOM, MIN_ZOOM};
    use crate::layout::layout_workload;
    use crate::model::{Batch, OccurrenceId, Workload};

    const SCENE: f64 = 1000.0;
    const STRIP: f64 = 1000.0;

    /// The layout of a workload whose batches have `sizes` systems. Tiles are 40 pixels high and
    /// a column holds 3 of them, so batches of more than 3 systems take several columns.
    fn layout(sizes: &[usize]) -> WorkloadLayout {
        let mut first = 0;
        let batches = sizes
            .iter()
            .map(|&size| {
                let others = (first..first + size).map(OccurrenceId).collect();
                first += size;
                Batch {
                    thread_bound: None,
                    others,
                    regroups: Vec::new(),
                }
            })
            .collect();
        let workload = Workload {
            name: "Workload(test)".to_string(),
            label: "test".to_string(),
            batches,
        };

        layout_workload(&workload, |_| 40.0, 200.0)
    }

    /// The layout of `count` batches of a single system.
    fn uniform(count: usize) -> WorkloadLayout {
        layout(&vec![1; count])
    }

    /// The camera at 100% with workload coordinate `x` in the middle of a scene `scene` wide.
    fn looking_at(x: f64, scene: f64) -> Camera {
        Camera {
            x: scene / 2.0 - x,
            zoom: 1.0,
        }
    }

    /// The camera at 100% with the lens on the middle of batch `index` and the gap after it.
    ///
    /// With the screen inside the workload, `width - SCENE` is off screen and the lens looks at
    /// `screen_left + SCENE * screen_left / (width - SCENE)`, that's `screen_left * width /
    /// (width - SCENE)`. We solve it for `screen_left`.
    fn centered_on(layout: &WorkloadLayout, index: usize) -> Camera {
        let start = layout.batches[index].bounds.x;
        let end = layout
            .batches
            .get(index + 1)
            .map_or(layout.width, |next| next.bounds.x);
        let focus = (start + end) / 2.0;

        let screen_left = focus * (layout.width - SCENE) / layout.width;

        Camera {
            x: -screen_left,
            zoom: 1.0,
        }
    }

    /// The index of the widest span.
    fn widest(lens: &Lens) -> usize {
        (0..lens.batches.len())
            .max_by(|&a, &b| lens.batches[a].width().total_cmp(&lens.batches[b].width()))
            .unwrap()
    }

    /// Asserts that the spans are finite, not empty, adjacent, and cover exactly `0..strip`.
    fn assert_covers(lens: &Lens, strip: f64) {
        let spans = &lens.batches;

        assert_eq!(spans.first().unwrap().left, 0.0);
        assert_eq!(spans.last().unwrap().right, strip);
        for span in spans {
            assert!(span.left.is_finite() && span.right.is_finite(), "{span:?}");
            assert!(span.width() > 0.0, "{span:?}");
        }
        for pair in spans.windows(2) {
            assert_eq!(pair[0].right, pair[1].left);
        }
    }

    /// Asserts that a workload without batches has nothing to draw.
    #[test]
    fn empty() {
        assert_eq!(lens(&layout(&[]), Camera::default(), SCENE, STRIP), None);
    }

    /// Asserts that unmeasured, empty, or non-finite widths and cameras give nothing to draw
    /// rather than non-finite spans.
    #[test]
    fn unusable() {
        let layout = uniform(3);
        let camera = Camera::default();

        for strip in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            assert_eq!(lens(&layout, camera, SCENE, strip), None);
        }
        for scene in [0.0, f64::NAN] {
            assert_eq!(lens(&layout, camera, scene, STRIP), None);
        }
        let lost = Camera {
            x: f64::NAN,
            zoom: 1.0,
        };
        assert_eq!(lens(&layout, lost, SCENE, STRIP), None);
    }

    /// Asserts that a single batch fills the strip, and so does the viewport when the batch is
    /// on screen.
    #[test]
    fn single() {
        let lens = lens(&layout(&[2]), Camera::default(), SCENE, STRIP).unwrap();
        let whole = Span {
            left: 0.0,
            right: STRIP,
        };

        assert_eq!(lens.batches, [whole]);
        assert_eq!(lens.viewport, Some(whole));
    }

    /// Asserts the widths of the selected lens, 45% uniform and 55% Gaussian with a spread of 4.2
    /// batches, on 60 batches, centered on batch 30. The superseded 35%/65% split would give 67.57
    /// and 5.83, the narrower 2.1 spread 111.98 for batch 30.
    #[test]
    fn reference() {
        let layout = uniform(60);

        let lens = lens(&layout, centered_on(&layout, 30), SCENE, STRIP).unwrap();

        let width = |index: usize| lens.batches[index].width();
        assert!((width(30) - 59.742).abs() < 1e-3, "{}", width(30));
        assert!((width(29) - 58.282).abs() < 1e-3, "{}", width(29));
        assert!((width(0) - 7.500).abs() < 1e-3, "{}", width(0));
    }

    /// Asserts that the batch under the lens is the widest, and that spans never widen away from
    /// it.
    #[test]
    fn magnifies() {
        let layout = uniform(60);

        let lens = lens(&layout, centered_on(&layout, 30), SCENE, STRIP).unwrap();

        // Far from the center, spans settle to the uniform share and only differ by rounding.
        let rounding = 1e-9;
        assert_eq!(widest(&lens), 30);
        for pair in lens.batches[..=30].windows(2) {
            assert!(pair[0].width() <= pair[1].width() + rounding);
        }
        for pair in lens.batches[30..].windows(2) {
            assert!(pair[0].width() + rounding >= pair[1].width());
        }
    }

    /// Asserts that spans cover narrow and wide strips exactly, from a few batches to thousands.
    #[test]
    fn covers() {
        for count in [3, 60, 240, 5000] {
            let layout = uniform(count);
            let camera = centered_on(&layout, count / 2);

            for strip in [40.0, STRIP, 4000.0] {
                assert_covers(&lens(&layout, camera, SCENE, strip).unwrap(), strip);
            }
        }
    }

    /// Asserts that wide and empty batches each keep a span covering the strip, wherever the
    /// camera is along the workload.
    #[test]
    fn unequal() {
        let layout = layout(&[1, 0, 12, 1, 0]);

        for step in -5..=25 {
            let camera = looking_at(step as f64 * 100.0, SCENE);

            let lens = lens(&layout, camera, SCENE, STRIP).unwrap();

            assert_eq!(lens.batches.len(), 5);
            assert_covers(&lens, STRIP);
        }
    }

    /// Asserts that thousands of batches stay finite and covered at both zoom limits.
    #[test]
    fn zoom_limits() {
        let layout = uniform(5000);

        for zoom in [MIN_ZOOM, MAX_ZOOM] {
            let camera = Camera::default().zoomed(zoom, 0.0);

            let lens = lens(&layout, camera, SCENE, STRIP).unwrap();

            assert_covers(&lens, STRIP);
            let viewport = lens.viewport.unwrap();
            assert!(0.0 <= viewport.left && viewport.left < viewport.right);
            assert!(viewport.right <= STRIP);
        }
    }

    /// Asserts that with the screen before the workload, the lens stays on the first batch and
    /// there's no viewport.
    #[test]
    fn before_start() {
        let camera = Camera {
            x: SCENE + 100.0,
            zoom: 1.0,
        };

        let lens = lens(&uniform(60), camera, SCENE, STRIP).unwrap();

        assert_covers(&lens, STRIP);
        assert_eq!(widest(&lens), 0);
        assert_eq!(lens.viewport, None);
    }

    /// Asserts that with the screen past the workload, the lens stays on the last batch and
    /// there's no viewport.
    #[test]
    fn past_end() {
        let layout = uniform(60);
        let camera = Camera {
            x: -layout.width - 100.0,
            zoom: 1.0,
        };

        let lens = lens(&layout, camera, SCENE, STRIP).unwrap();

        assert_covers(&lens, STRIP);
        assert_eq!(widest(&lens), 59);
        assert_eq!(lens.viewport, None);
    }

    /// Asserts that with the workload's start framed, the first batch gets the lens' full size,
    /// at least as wide as the middle batch under a centered lens, although several batches are
    /// on screen.
    #[test]
    fn start() {
        let layout = uniform(61);

        let start = lens(&layout, Camera::default(), SCENE, STRIP).unwrap();
        let middle = lens(&layout, centered_on(&layout, 30), SCENE, STRIP).unwrap();

        assert_eq!(widest(&start), 0);
        assert!(start.batches[0].width() >= middle.batches[30].width());
    }

    /// Asserts that with the slider at the workload's end, the last batch gets the lens' full
    /// size.
    #[test]
    fn end() {
        let layout = uniform(61);
        let camera = Camera::default().at_position(1.0, layout.width, SCENE);

        let end = lens(&layout, camera, SCENE, STRIP).unwrap();
        let middle = lens(&layout, centered_on(&layout, 30), SCENE, STRIP).unwrap();

        assert_eq!(widest(&end), 60);
        assert!(end.batches[60].width() >= middle.batches[30].width());
    }

    /// Asserts that with the whole workload on screen, the lens is on its middle batch rather than
    /// an end.
    #[test]
    fn whole_middle() {
        let camera = Camera::default().zoomed(MIN_ZOOM, 0.0);

        let lens = lens(&uniform(61), camera, SCENE, STRIP).unwrap();

        assert_eq!(widest(&lens), 30);
    }

    /// Asserts that the workload's ends land on the strip's ends: a screen showing the whole
    /// workload gets the whole strip as viewport.
    #[test]
    fn whole_viewport() {
        let camera = Camera::default().zoomed(MIN_ZOOM, 0.0);

        let lens = lens(&uniform(60), camera, SCENE, STRIP).unwrap();

        let whole = Span {
            left: 0.0,
            right: STRIP,
        };
        assert_eq!(lens.viewport, Some(whole));
    }

    /// Asserts that the viewport surrounds the batch at the screen's center, inside the strip.
    #[test]
    fn viewport_inside() {
        let layout = uniform(60);

        let lens = lens(&layout, centered_on(&layout, 30), SCENE, STRIP).unwrap();

        let viewport = lens.viewport.unwrap();
        let center = lens.batches[30];
        assert!(0.0 < viewport.left && viewport.left < center.left);
        assert!(center.right < viewport.right && viewport.right < STRIP);
    }

    /// Asserts that with the workload starting mid-screen, the viewport starts at the strip's left
    /// edge and ends in the batch at the screen's right edge.
    #[test]
    fn viewport_partial() {
        let layout = uniform(60);
        let camera = Camera {
            x: 600.0,
            zoom: 1.0,
        };

        let lens = lens(&layout, camera, SCENE, STRIP).unwrap();

        // The screen's right edge is at workload coordinate 400, in batch 1.
        let viewport = lens.viewport.unwrap();
        let batch = lens.batches[1];
        assert_eq!(viewport.left, 0.0);
        assert!(batch.left < viewport.right && viewport.right < batch.right);
    }

    /// Asserts that positions count the gap after a batch as part of it, and that the workload's
    /// end is at the batch count.
    #[test]
    fn position_intervals() {
        let layout = layout(&[1, 12, 1]);
        let wide = layout.batches[1].bounds.x;
        let next = layout.batches[2].bounds.x;

        assert_eq!(batch_position(&layout, 0.0), 0.0);
        assert_eq!(batch_position(&layout, wide), 1.0);
        assert_eq!(batch_position(&layout, (wide + next) / 2.0), 1.5);
        assert_eq!(batch_position(&layout, layout.width), 3.0);
    }

    /// Asserts that the lens barely moves when the screen's center crosses into the next batch,
    /// so spans don't jump at batch boundaries.
    #[test]
    fn continuous() {
        let layout = layout(&[1, 12, 1]);
        let boundary = layout.batches[1].bounds.x;
        // A narrow scene, so the screen's edges stay inside the workload.
        let scene = 100.0;

        let before = lens(&layout, looking_at(boundary - 0.01, scene), scene, STRIP).unwrap();
        let after = lens(&layout, looking_at(boundary + 0.01, scene), scene, STRIP).unwrap();

        for (a, b) in before.batches.iter().zip(&after.batches) {
            assert!((a.left - b.left).abs() < 0.01 && (a.right - b.right).abs() < 0.01);
        }
    }
}
