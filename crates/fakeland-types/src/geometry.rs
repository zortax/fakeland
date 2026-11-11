//! Points, sizes and rectangles in logical or physical coordinate spaces.
//!
//! Logical coordinates are the compositor space that windows and input use. Physical coordinates are
//! buffer pixels: logical times the output scale.

use std::fmt;
use std::marker::PhantomData;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A coordinate space marker.
pub trait Space:
    Copy + Default + fmt::Debug + PartialEq + JsonSchema + Send + Sync + 'static
{
}

/// The logical compositor space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, JsonSchema)]
pub struct Logical;

/// The physical pixel space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, JsonSchema)]
pub struct Physical;

impl Space for Logical {}
impl Space for Physical {}

/// A point with fractional coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(bound = "")]
pub struct Point<S: Space> {
    /// Horizontal coordinate.
    pub x: f64,
    /// Vertical coordinate.
    pub y: f64,
    #[serde(skip)]
    space: PhantomData<S>,
}

/// A size in whole units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(bound = "")]
pub struct Size<S: Space> {
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
    #[serde(skip)]
    space: PhantomData<S>,
}

/// An axis-aligned rectangle in whole units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(bound = "")]
pub struct Rect<S: Space> {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
    #[serde(skip)]
    space: PhantomData<S>,
}

impl<S: Space> Point<S> {
    /// Builds a point.
    pub const fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            space: PhantomData,
        }
    }

    /// Returns the point moved by an offset.
    pub fn offset(self, dx: f64, dy: f64) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    /// Returns the linear interpolation between `self` and `other` at `t` in `0..=1`.
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
        )
    }

    /// Returns the euclidean distance to another point.
    pub fn distance(self, other: Self) -> f64 {
        ((other.x - self.x).powi(2) + (other.y - self.y).powi(2)).sqrt()
    }
}

impl<S: Space> Size<S> {
    /// Builds a size.
    pub const fn new(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            space: PhantomData,
        }
    }

    /// Returns the area.
    pub const fn area(self) -> i64 {
        self.w as i64 * self.h as i64
    }
}

impl<S: Space> Rect<S> {
    /// Builds a rectangle.
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            x,
            y,
            w,
            h,
            space: PhantomData,
        }
    }

    /// Builds a rectangle at the origin.
    pub const fn from_size(size: Size<S>) -> Self {
        Self::new(0, 0, size.w, size.h)
    }

    /// Returns the size.
    pub const fn size(self) -> Size<S> {
        Size::new(self.w, self.h)
    }

    /// Returns the area.
    pub const fn area(self) -> i64 {
        self.w as i64 * self.h as i64
    }

    /// Returns true for a rectangle without area.
    pub const fn is_empty(self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    /// Returns the center point.
    pub fn center(self) -> Point<S> {
        Point::new(
            self.x as f64 + self.w as f64 / 2.0,
            self.y as f64 + self.h as f64 / 2.0,
        )
    }

    /// Returns true when the point lies inside.
    pub fn contains(self, p: Point<S>) -> bool {
        p.x >= self.x as f64
            && p.y >= self.y as f64
            && p.x < (self.x + self.w) as f64
            && p.y < (self.y + self.h) as f64
    }

    /// Returns the overlap of two rectangles, or `None` when they do not overlap.
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = (self.x + self.w).min(other.x + other.w);
        let y1 = (self.y + self.h).min(other.y + other.h);
        (x1 > x0 && y1 > y0).then(|| Self::new(x0, y0, x1 - x0, y1 - y0))
    }

    /// Returns true when the rectangles overlap.
    pub fn intersects(self, other: Self) -> bool {
        self.intersection(other).is_some()
    }

    /// Returns the smallest rectangle that holds both.
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.w).max(other.x + other.w);
        let y1 = (self.y + self.h).max(other.y + other.h);
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// Returns the rectangle moved by an offset.
    pub const fn translate(self, dx: i32, dy: i32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

impl Point<Logical> {
    /// Converts to physical pixels at a scale.
    pub fn to_physical(self, scale: f64) -> Point<Physical> {
        Point::new(self.x * scale, self.y * scale)
    }
}

impl Point<Physical> {
    /// Converts to logical coordinates at a scale.
    pub fn to_logical(self, scale: f64) -> Point<Logical> {
        Point::new(self.x / scale, self.y / scale)
    }
}

impl Rect<Logical> {
    /// Converts to physical pixels at a scale. The edges round outward.
    pub fn to_physical(self, scale: f64) -> Rect<Physical> {
        let x0 = (self.x as f64 * scale).floor() as i32;
        let y0 = (self.y as f64 * scale).floor() as i32;
        let x1 = ((self.x + self.w) as f64 * scale).ceil() as i32;
        let y1 = ((self.y + self.h) as f64 * scale).ceil() as i32;
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

impl Rect<Physical> {
    /// Converts to logical coordinates at a scale. The edges round outward.
    pub fn to_logical(self, scale: f64) -> Rect<Logical> {
        let x0 = (self.x as f64 / scale).floor() as i32;
        let y0 = (self.y as f64 / scale).floor() as i32;
        let x1 = ((self.x + self.w) as f64 / scale).ceil() as i32;
        let y1 = ((self.y + self.h) as f64 / scale).ceil() as i32;
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

impl Size<Logical> {
    /// Converts to physical pixels at a scale, rounded to the nearest pixel.
    pub fn to_physical(self, scale: f64) -> Size<Physical> {
        Size::new(
            (self.w as f64 * scale).round() as i32,
            (self.h as f64 * scale).round() as i32,
        )
    }
}

impl Size<Physical> {
    /// Converts to logical coordinates at a scale, rounded to the nearest unit.
    pub fn to_logical(self, scale: f64) -> Size<Logical> {
        Size::new(
            (self.w as f64 / scale).round() as i32,
            (self.h as f64 / scale).round() as i32,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_and_union() {
        let a = Rect::<Logical>::new(0, 0, 10, 10);
        let b = Rect::<Logical>::new(5, 5, 10, 10);
        assert_eq!(a.intersection(b), Some(Rect::new(5, 5, 5, 5)));
        assert_eq!(a.union(b), Rect::new(0, 0, 15, 15));
        assert_eq!(a.intersection(Rect::new(20, 20, 1, 1)), None);
    }

    #[test]
    fn fractional_scale_rounds_outward() {
        let r = Rect::<Logical>::new(1, 1, 3, 3).to_physical(1.5);
        assert_eq!(r, Rect::new(1, 1, 5, 5));
    }

    #[test]
    fn serde_skips_the_space() {
        let p = Point::<Logical>::new(1.5, 2.0);
        let json = serde_json::to_string(&p).unwrap_or_default();
        assert_eq!(json, r#"{"x":1.5,"y":2.0}"#);
    }
}
