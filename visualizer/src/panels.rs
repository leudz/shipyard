//! Where floating panels go: around what opened them, moved and resized by the user, and always
//! kept inside the scene.
//!
//! Coordinates are CSS pixels from the scene's top left corner.

/// The space kept between a panel and the scene's edges.
pub const MARGIN: f64 = 8.0;
/// The smallest size resizing allows, unless the scene itself is smaller.
pub const MIN_WIDTH: f64 = 240.0;
pub const MIN_HEIGHT: f64 = 120.0;
/// How far above the pointer an opened panel starts, so its header is next to the pointer.
const POINTER_RISE: f64 = 40.0;
/// Where the first panel opened without a pointer goes, below the search panel.
const CASCADE_TOP: f64 = 100.0;
const CASCADE_RIGHT: f64 = 14.0;
/// How far each later panel opened without a pointer is moved down and left.
const CASCADE_STEP: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

/// A panel's position and size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    /// `None` until the user resizes it: as tall as its content, up to [`Panel::max_height`].
    pub height: Option<f64>,
}

impl Panel {
    /// A panel opened by a click at `pointer`: centered on it horizontally, its header next to it.
    pub fn opened_at(pointer: Point, width: f64, scene: Size) -> Panel {
        Panel {
            x: pointer.x - width / 2.0,
            y: pointer.y - POINTER_RISE,
            width,
            height: None,
        }
        .fitted(scene)
    }

    /// A panel opened without a pointer, the `index`th one: cascading from the top right corner.
    pub fn cascaded(index: usize, width: f64, scene: Size) -> Panel {
        let offset = index as f64 * CASCADE_STEP;

        Panel {
            x: scene.width - CASCADE_RIGHT - offset - width,
            y: CASCADE_TOP + offset,
            width,
            height: None,
        }
        .fitted(scene)
    }

    /// The panel moved to be inside `scene`, and shrunk when it's larger than the scene.
    ///
    /// A panel without a height keeps at least [`MIN_HEIGHT`] on screen.
    pub fn fitted(self, scene: Size) -> Panel {
        let room = Size {
            width: (scene.width - 2.0 * MARGIN).max(1.0),
            height: (scene.height - 2.0 * MARGIN).max(1.0),
        };
        let width = self.width.min(room.width);
        let height = self.height.map(|height| height.min(room.height));
        let shown_height = height.unwrap_or(MIN_HEIGHT.min(room.height));

        // `max` after `min`, so a scene too small for the panel keeps its top left corner visible.
        Panel {
            x: self.x.min(scene.width - MARGIN - width).max(MARGIN),
            y: self.y.min(scene.height - MARGIN - shown_height).max(MARGIN),
            width,
            height,
        }
    }

    /// The panel moved by `dx` and `dy`, staying inside `scene`.
    ///
    /// A panel as tall as its content stops when its `rendered_height` reaches the bottom, instead
    /// of shrinking to go further down.
    pub fn moved(self, dx: f64, dy: f64, rendered_height: f64, scene: Size) -> Panel {
        if !dx.is_finite() || !dy.is_finite() {
            return self;
        }

        let moved = Panel {
            x: self.x + dx,
            y: self.y + dy,
            ..self
        }
        .fitted(scene);
        if self.height.is_some() {
            return moved;
        }

        // `fitted` only keeps the minimum height on screen. The panel was already rendered at
        // most this tall at its position, so the limit never pushes it up.
        let lowest = scene.height - MARGIN - rendered_height;
        Panel {
            y: moved.y.min(lowest).max(MARGIN),
            ..moved
        }
    }

    /// The panel resized to `width` and `height`, keeping its top left corner.
    ///
    /// The size stays between the minimum and what fits below and right of that corner.
    pub fn resized(self, width: f64, height: f64, scene: Size) -> Panel {
        if !width.is_finite() || !height.is_finite() {
            return self;
        }

        let available_width = (scene.width - MARGIN - self.x).max(1.0);
        let available_height = (scene.height - MARGIN - self.y).max(1.0);

        Panel {
            width: width.max(MIN_WIDTH).min(available_width),
            height: Some(height.max(MIN_HEIGHT).min(available_height)),
            ..self
        }
    }

    /// The tallest the panel can be at its position.
    pub fn max_height(self, scene: Size) -> f64 {
        (scene.height - MARGIN - self.y).max(1.0)
    }
}

/// How wide a new panel titled `title` is: wide enough for the title, within limits.
pub fn preferred_width(title: &str) -> f64 {
    let title_width = title.chars().count() as f64 * 8.0 + 40.0;

    title_width.clamp(340.0, 440.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCENE: Size = Size {
        width: 1000.0,
        height: 700.0,
    };

    fn panel(x: f64, y: f64, width: f64, height: Option<f64>) -> Panel {
        Panel {
            x,
            y,
            width,
            height,
        }
    }

    /// Asserts that a panel opens centered on the pointer horizontally, its header next to it.
    #[test]
    fn opened() {
        let opened = Panel::opened_at(Point { x: 300.0, y: 400.0 }, 340.0, SCENE);

        assert_eq!(opened, panel(130.0, 360.0, 340.0, None));
    }

    /// Asserts that a panel opened near the right edge moves left to stay inside the scene.
    #[test]
    fn opened_right_edge() {
        let opened = Panel::opened_at(Point { x: 950.0, y: 400.0 }, 340.0, SCENE);

        assert_eq!(opened.x, 1000.0 - MARGIN - 340.0);
    }

    /// Asserts that a panel opened near the top left corner stays inside the scene.
    #[test]
    fn opened_corner() {
        let opened = Panel::opened_at(Point { x: 0.0, y: 0.0 }, 340.0, SCENE);

        assert_eq!((opened.x, opened.y), (MARGIN, MARGIN));
    }

    /// Asserts that panels opened without a pointer cascade from the top right corner.
    #[test]
    fn cascade() {
        let first = Panel::cascaded(0, 340.0, SCENE);
        let second = Panel::cascaded(1, 340.0, SCENE);

        assert_eq!(first, panel(1000.0 - 14.0 - 340.0, 100.0, 340.0, None));
        assert_eq!((second.x, second.y), (first.x - 24.0, first.y + 24.0));
    }

    /// Asserts that a panel inside the scene doesn't move.
    #[test]
    fn fitted_inside() {
        let inside = panel(100.0, 100.0, 340.0, Some(300.0));

        assert_eq!(inside.fitted(SCENE), inside);
    }

    /// Asserts that a panel past the bottom right corner, after the scene shrank, moves back in.
    #[test]
    fn fitted_back() {
        let outside = panel(900.0, 650.0, 340.0, Some(300.0));

        let fitted = outside.fitted(SCENE);

        assert_eq!(
            fitted,
            panel(
                1000.0 - 8.0 - 340.0,
                700.0 - 8.0 - 300.0,
                340.0,
                Some(300.0)
            )
        );
    }

    /// Asserts that a panel without a height keeps its minimum height on screen.
    #[test]
    fn fitted_auto_height() {
        let low = panel(100.0, 690.0, 340.0, None);

        assert_eq!(low.fitted(SCENE).y, 700.0 - 8.0 - MIN_HEIGHT);
    }

    /// Asserts that a panel larger than the scene shrinks to the scene, margins included.
    #[test]
    fn fitted_large() {
        let small = Size {
            width: 300.0,
            height: 200.0,
        };

        let fitted = panel(50.0, 50.0, 340.0, Some(500.0)).fitted(small);

        assert_eq!(fitted, panel(MARGIN, MARGIN, 284.0, Some(184.0)));
    }

    /// Asserts that a scene smaller than the margins still gives a visible, finite panel.
    #[test]
    fn fitted_tiny() {
        let tiny = Size {
            width: 10.0,
            height: 10.0,
        };

        let fitted = panel(50.0, 50.0, 340.0, None).fitted(tiny);

        assert_eq!(fitted, panel(MARGIN, MARGIN, 1.0, None));
    }

    /// Asserts that moving stops at the scene's edges.
    #[test]
    fn moved() {
        let start = panel(100.0, 100.0, 340.0, Some(300.0));

        assert_eq!(
            start.moved(50.0, -20.0, 300.0, SCENE),
            panel(150.0, 80.0, 340.0, Some(300.0))
        );
        assert_eq!(
            start.moved(-500.0, -500.0, 300.0, SCENE),
            panel(MARGIN, MARGIN, 340.0, Some(300.0))
        );
    }

    /// Asserts that a panel as tall as its content, moved down past the bottom, stops with its
    /// whole rendered height on screen instead of shrinking.
    #[test]
    fn moved_keeps_height() {
        let start = panel(100.0, 100.0, 340.0, None);

        let moved = start.moved(0.0, 500.0, 250.0, SCENE);

        assert_eq!(moved.y, 700.0 - MARGIN - 250.0);
    }

    /// Asserts that a non-finite move, from a broken event, leaves the panel alone.
    #[test]
    fn moved_nan() {
        let start = panel(100.0, 100.0, 340.0, None);

        assert_eq!(start.moved(f64::NAN, 0.0, 250.0, SCENE), start);
    }

    /// Asserts that resizing gives the panel a height and keeps its corner.
    #[test]
    fn resized() {
        let start = panel(100.0, 100.0, 340.0, None);

        assert_eq!(
            start.resized(400.0, 300.0, SCENE),
            panel(100.0, 100.0, 400.0, Some(300.0))
        );
    }

    /// Asserts that resizing stays between the minimum size and the scene's edges.
    #[test]
    fn resized_limits() {
        let start = panel(600.0, 400.0, 340.0, Some(200.0));

        assert_eq!(
            start.resized(10.0, 10.0, SCENE),
            panel(600.0, 400.0, MIN_WIDTH, Some(MIN_HEIGHT))
        );
        assert_eq!(
            start.resized(5000.0, 5000.0, SCENE),
            panel(600.0, 400.0, 392.0, Some(292.0))
        );
    }

    /// Asserts that a panel without a height can grow down to the scene's bottom margin.
    #[test]
    fn max_height() {
        assert_eq!(panel(100.0, 100.0, 340.0, None).max_height(SCENE), 592.0);
    }

    /// Asserts that new panels fit their title, between the narrowest and widest default.
    #[test]
    fn preferred() {
        assert_eq!(preferred_width("run"), 340.0);
        assert_eq!(preferred_width(&"a".repeat(45)), 400.0);
        assert_eq!(preferred_width(&"a".repeat(100)), 440.0);
    }
}
