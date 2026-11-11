//! How the workload scene is panned and zoomed.
//!
//! The camera only moves horizontally: the workload stays anchored below the scene's heading and
//! its columns reflow to the height left at each zoom.

/// The space kept left of the workload when it's framed, in screen pixels.
pub const MARGIN: f64 = 25.0;
pub const MIN_ZOOM: f64 = 0.03;
pub const MAX_ZOOM: f64 = 1.6;
/// How much one zoom button press zooms in or out.
pub const ZOOM_STEP: f64 = 1.25;
/// How far, in screen pixels, a press can move and still count as a click rather than a drag.
pub const CLICK_SLOP: f64 = 4.0;

/// Returns `true` if a press that moved by `(dx, dy)` screen pixels is a drag, not a click.
pub fn is_drag(dx: f64, dy: f64) -> bool {
    let distance = dx.hypot(dy);

    // A NaN distance can't be trusted to be a click.
    distance.is_nan() || distance > CLICK_SLOP
}

/// Where the scene is drawn: scene point `p` lands on screen at `x + p * zoom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub x: f64,
    pub zoom: f64,
}

impl Default for Camera {
    /// The workload's start at 100%.
    fn default() -> Camera {
        Camera {
            x: MARGIN,
            zoom: 1.0,
        }
    }
}

impl Camera {
    /// Where scene coordinate `scene_x` is on screen.
    pub fn to_screen(self, scene_x: f64) -> f64 {
        self.x + scene_x * self.zoom
    }

    /// Moves the scene right by `dx` screen pixels.
    pub fn panned(self, dx: f64) -> Camera {
        if !dx.is_finite() {
            return self;
        }

        Camera {
            x: self.x + dx,
            ..self
        }
    }

    /// Changes the zoom, clamped to [`MIN_ZOOM`]..=[`MAX_ZOOM`], keeping the scene point under
    /// screen coordinate `anchor` in place.
    pub fn zoomed(self, zoom: f64, anchor: f64) -> Camera {
        if !zoom.is_finite() || !anchor.is_finite() {
            return self;
        }

        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let scene_anchor = (anchor - self.x) / self.zoom;

        Camera {
            x: anchor - scene_anchor * zoom,
            zoom,
        }
    }

    /// Puts scene coordinate `scene_x` at the left margin.
    pub fn framing(self, scene_x: f64) -> Camera {
        Camera {
            x: MARGIN - scene_x * self.zoom,
            ..self
        }
    }

    /// Pans as little as possible so the scene span `left..right` is on screen, margins included.
    ///
    /// A span wider than the screen gets its left side shown.
    pub fn revealing(self, left: f64, right: f64, viewport_width: f64) -> Camera {
        let screen_left = self.to_screen(left);
        let screen_right = self.to_screen(right);

        if screen_left < MARGIN {
            return self.framing(left);
        }
        if screen_right > viewport_width - MARGIN {
            let panned = self.panned(viewport_width - MARGIN - screen_right);
            // Showing the right side mustn't hide the left one.
            return if panned.to_screen(left) < MARGIN {
                self.framing(left)
            } else {
                panned
            };
        }

        self
    }

    /// Whether any of the scene span `left..right` is on a screen `viewport_width` wide.
    pub fn shows(self, left: f64, right: f64, viewport_width: f64) -> bool {
        self.to_screen(right) > 0.0 && self.to_screen(left) < viewport_width
    }

    /// How far along a workload `layout_width` wide the camera is, from 0 with its start at the
    /// left margin to 1 with its end at the right margin.
    pub fn position(self, layout_width: f64, viewport_width: f64) -> f64 {
        let travel = travel(layout_width * self.zoom, viewport_width);

        ((MARGIN - self.x) / travel).clamp(0.0, 1.0)
    }

    /// The camera at `position` along a workload `layout_width` wide, see [`Camera::position`].
    pub fn at_position(self, position: f64, layout_width: f64, viewport_width: f64) -> Camera {
        if !position.is_finite() {
            return self;
        }

        let travel = travel(layout_width * self.zoom, viewport_width);

        Camera {
            x: MARGIN - position.clamp(0.0, 1.0) * travel,
            ..self
        }
    }
}

/// How far the camera pans from a workload's start to its end, at least a pixel so positions
/// stay finite for workloads narrower than the screen.
fn travel(scaled_width: f64, viewport_width: f64) -> f64 {
    (scaled_width - viewport_width + 2.0 * MARGIN).max(1.0)
}

/// The camera showing the whole workload: the largest zoom up to 100% whose layout fits
/// `viewport_width` with a margin on each side.
///
/// `layout_width` gives the workload's width in scene pixels when laid out at a zoom. Zooming
/// out leaves more scene height for columns, so the width changes with the zoom.
pub fn fit(viewport_width: f64, layout_width: impl Fn(f64) -> f64) -> Camera {
    let room = viewport_width - 2.0 * MARGIN;
    let fits = |zoom: f64| layout_width(zoom) * zoom <= room;

    if fits(1.0) {
        return Camera::default();
    }

    // `low` always fits, unless nothing does and we settle for the smallest zoom.
    // Halving the gap 30 times is far below a visible difference.
    let (mut low, mut high) = (MIN_ZOOM, 1.0);
    for _ in 0..30 {
        let middle = (low + high) / 2.0;
        if fits(middle) {
            low = middle;
        } else {
            high = middle;
        }
    }

    Camera {
        x: MARGIN,
        zoom: low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asserts that a press moving a few pixels is still a click, and one moving further is a drag.
    #[test]
    fn click_or_drag() {
        assert!(!is_drag(0.0, 0.0));
        assert!(!is_drag(3.0, -2.0));
        assert!(is_drag(6.0, 0.0));
        assert!(is_drag(0.0, -10.0));
        assert!(is_drag(f64::NAN, 0.0));
    }

    fn camera(x: f64, zoom: f64) -> Camera {
        Camera { x, zoom }
    }

    /// Asserts that panning moves the scene by the given screen distance.
    #[test]
    fn pan() {
        assert_eq!(camera(25.0, 0.5).panned(-100.0), camera(-75.0, 0.5));
    }

    /// Asserts that a non-finite pan, from a broken event, leaves the camera alone.
    #[test]
    fn pan_nan() {
        assert_eq!(camera(25.0, 1.0).panned(f64::NAN), camera(25.0, 1.0));
    }

    /// Asserts that zooming keeps the scene point under the anchor in place.
    #[test]
    fn zoom_anchor() {
        let before = camera(25.0, 1.0);

        let after = before.zoomed(0.5, 425.0);

        // Scene point 400 was under 425 and still is.
        assert_eq!(after.to_screen(400.0), 425.0);
        assert_eq!(after.zoom, 0.5);
    }

    /// Asserts that zoom stays between the minimum and maximum.
    #[test]
    fn zoom_clamped() {
        assert_eq!(camera(25.0, 1.0).zoomed(100.0, 0.0).zoom, MAX_ZOOM);
        assert_eq!(camera(25.0, 1.0).zoomed(0.0, 0.0).zoom, MIN_ZOOM);
    }

    /// Asserts that framing puts the scene coordinate at the margin, at the current zoom.
    #[test]
    fn frame() {
        let framed = camera(-500.0, 0.5).framing(800.0);

        assert_eq!(framed.to_screen(800.0), MARGIN);
        assert_eq!(framed.zoom, 0.5);
    }

    /// Asserts that a span already on screen doesn't move the camera.
    #[test]
    fn reveal_visible() {
        let camera = camera(25.0, 1.0);

        assert_eq!(camera.revealing(100.0, 340.0, 1000.0), camera);
    }

    /// Asserts that a span past the right edge is panned in just enough.
    #[test]
    fn reveal_right() {
        let revealed = camera(25.0, 1.0).revealing(1000.0, 1240.0, 1000.0);

        assert_eq!(revealed.to_screen(1240.0), 1000.0 - MARGIN);
    }

    /// Asserts that a span past the left edge is framed at the margin.
    #[test]
    fn reveal_left() {
        let revealed = camera(-2000.0, 1.0).revealing(100.0, 340.0, 1000.0);

        assert_eq!(revealed.to_screen(100.0), MARGIN);
    }

    /// Asserts that a span wider than the screen shows its left side.
    #[test]
    fn reveal_wide() {
        let revealed = camera(25.0, 1.0).revealing(1000.0, 3000.0, 1000.0);

        assert_eq!(revealed.to_screen(1000.0), MARGIN);
    }

    /// Asserts that a workload fitting at 100% stays at 100%.
    #[test]
    fn fit_small() {
        assert_eq!(fit(1000.0, |_| 500.0), Camera::default());
    }

    /// Asserts that an empty workload fits at 100%.
    #[test]
    fn fit_empty() {
        assert_eq!(fit(1000.0, |_| 0.0), Camera::default());
    }

    /// Asserts that a wide workload zooms out until it fits.
    #[test]
    fn fit_wide() {
        let width = |_| 4000.0;

        let fitted = fit(1050.0, width);

        assert!(fitted.zoom < 1.0);
        assert!(width(fitted.zoom) * fitted.zoom <= 1000.0);
        assert!((width(fitted.zoom) * fitted.zoom - 1000.0).abs() < 1.0);
    }

    /// Asserts that fitting accounts for the layout narrowing as columns get taller.
    #[test]
    fn fit_reflow() {
        // Each halving of the zoom halves the width, like columns holding twice the tiles.
        let width = |zoom: f64| 4000.0 * zoom;

        let fitted = fit(1050.0, width);

        assert!(width(fitted.zoom) * fitted.zoom <= 1000.0);
        assert!((fitted.zoom - 0.5).abs() < 0.001);
    }

    /// Asserts that a workload too wide for even the smallest zoom settles for it.
    #[test]
    fn fit_huge() {
        assert_eq!(fit(1000.0, |_| 1e9).zoom, MIN_ZOOM);
    }
    /// Asserts that a span is shown when any of it is on screen.
    #[test]
    fn shows() {
        let camera = camera(-100.0, 1.0);

        assert!(camera.shows(50.0, 150.0, 1000.0));
        assert!(!camera.shows(0.0, 90.0, 1000.0));
        assert!(!camera.shows(1200.0, 1300.0, 1000.0));
    }

    /// Asserts that the position goes from 0 at the workload's start to 1 at its end.
    #[test]
    fn position_ends() {
        let start = Camera::default();
        // 3000 wide on a 1000 wide screen: the end is at the right margin at x = -1975.
        let end = camera(1000.0 - MARGIN - 3000.0, 1.0);

        assert_eq!(start.position(3000.0, 1000.0), 0.0);
        assert_eq!(end.position(3000.0, 1000.0), 1.0);
    }

    /// Asserts that moving to a position and reading it back gives the same position, zoomed.
    #[test]
    fn position_round_trip() {
        let moved = camera(25.0, 0.5).at_position(0.25, 6000.0, 1000.0);

        assert!((moved.position(6000.0, 1000.0) - 0.25).abs() < 1e-9);
        assert_eq!(moved.zoom, 0.5);
    }

    /// Asserts that a workload narrower than the screen, or empty, keeps finite positions.
    #[test]
    fn position_narrow() {
        assert_eq!(Camera::default().position(0.0, 1000.0), 0.0);
        assert!(Camera::default()
            .at_position(1.0, 0.0, 1000.0)
            .x
            .is_finite());
    }
}
