//! The set of selected scenes. It routes input effects to every scene, tracks whether the
//! picture changed, and draws the scenes in a fixed order: background, scroll list, click target,
//! size display, moving bar, hover target, glyph boxes, frame id and pointer marker.

use fakeland_types::{Logical, Point, Size, Timestamp};
use serde::Serialize;

use super::{
    AnimateScene, ClickScene, HoverScene, ResizeScene, SceneKind, SceneOptions, ScrollScene,
    TypeScene,
};
use crate::draw::{Canvas, Color, GlyphBoxStyle, draw_text, text_height};
use crate::input::InputEffect;

/// Logical size of one font pixel of the frame id.
const FRAME_ID_CELL: f64 = 2.0;
/// Logical half size of the pointer marker.
const MARKER: f64 = 6.0;

/// What a frame shows, for the event log.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct FrameState {
    /// The left edge of the moving bar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar_x: Option<f64>,
    /// The unwrapped distance of the moving bar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bar_distance: Option<f64>,
    /// The scroll offset `[x, y]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll: Option<[f64; 2]>,
    /// The pointer is inside the hover target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover: Option<bool>,
    /// The click target is toggled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toggled: Option<bool>,
    /// The number of presses in the click target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presses: Option<u64>,
    /// The number of typed characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_len: Option<usize>,
}

/// The inputs of one draw.
#[derive(Clone, Copy, Debug)]
pub struct DrawContext {
    /// The logical window size.
    pub window: Size<Logical>,
    /// The id of the frame.
    pub frame_id: u64,
    /// The animation time of the frame.
    pub time: Timestamp,
}

/// The selected scenes and their state.
#[derive(Debug)]
pub struct SceneSet {
    background: Color,
    hover: Option<HoverScene>,
    click: Option<ClickScene>,
    scroll: Option<ScrollScene>,
    typing: Option<TypeScene>,
    animate: Option<AnimateScene>,
    resize: Option<ResizeScene>,
    idle: bool,
    pointer_marker: bool,
    show_frame_id: bool,
    pointer: Option<Point<Logical>>,
}

impl SceneSet {
    /// Builds the selected scenes.
    pub fn new(o: &SceneOptions) -> Self {
        let pick = |k| o.has(k);
        Self {
            background: o.background,
            hover: pick(SceneKind::Hover)
                .then(|| HoverScene::new(o.hover_rect, o.hover_color, o.hover_active_color)),
            click: pick(SceneKind::Click)
                .then(|| ClickScene::new(o.click_rect, o.click_color, o.click_active_color)),
            scroll: pick(SceneKind::Scroll).then(|| {
                ScrollScene::new(
                    o.scroll_unit,
                    o.wheel_step,
                    o.scroll_factor,
                    o.stripe_height,
                )
            }),
            typing: pick(SceneKind::Type).then(|| TypeScene::new(GlyphBoxStyle::default())),
            animate: pick(SceneKind::Animate)
                .then(|| AnimateScene::new(o.speed, o.bar_width, o.bar_color)),
            resize: pick(SceneKind::Resize).then(|| ResizeScene::new(Color::WHITE)),
            idle: pick(SceneKind::Idle),
            pointer_marker: o.pointer_marker,
            show_frame_id: o.show_frame_id,
            pointer: None,
        }
    }

    /// Returns true when the picture changes with time alone.
    pub fn animating(&self) -> bool {
        self.animate.is_some()
    }

    /// Returns true for the idle scene, which draws only for configures.
    pub fn idle(&self) -> bool {
        self.idle
    }

    /// Returns true when the type scene is selected.
    pub fn wants_text(&self) -> bool {
        self.typing.is_some()
    }

    /// Applies an input effect to every scene. Returns true when the picture changed.
    pub fn apply(&mut self, effect: &InputEffect, window: Size<Logical>) -> bool {
        let mut changed = false;
        if self.pointer_marker {
            changed |= self.track_pointer(effect);
        }
        if let Some(s) = &mut self.hover {
            changed |= s.apply(effect);
        }
        if let Some(s) = &mut self.click {
            changed |= s.apply(effect, window);
        }
        if let Some(s) = &mut self.scroll {
            changed |= s.apply(effect);
        }
        if let Some(s) = &mut self.typing {
            changed |= s.apply(effect);
        }
        changed
    }

    /// Draws a full frame and returns what it shows.
    pub fn draw(&mut self, canvas: &mut Canvas<'_>, ctx: &DrawContext) -> FrameState {
        let mut state = FrameState::default();
        canvas.clear(self.background);
        if let Some(s) = &self.scroll {
            s.draw(canvas);
            let (x, y) = s.offset();
            state.scroll = Some([x, y]);
        }
        if let Some(s) = &self.click {
            s.draw(canvas, ctx.window);
            state.toggled = Some(s.toggled());
            state.presses = Some(s.presses());
        }
        if let Some(s) = &self.resize {
            s.draw(canvas, ctx.window);
        }
        if let Some(s) = &mut self.animate {
            state.bar_x = Some(s.draw(canvas, ctx.time));
            state.bar_distance = Some(s.distance(ctx.time));
        }
        if let Some(s) = &self.hover {
            s.draw(canvas);
            state.hover = Some(s.inside());
        }
        if let Some(s) = &self.typing {
            s.draw(canvas);
            state.text_len = Some(s.text().chars().count());
        }
        if self.show_frame_id {
            let y = ctx.window.h as f64 - text_height(FRAME_ID_CELL) - 6.0;
            let id = ctx.frame_id.to_string();
            draw_text(canvas, 6.0, y, FRAME_ID_CELL, &id, Color::WHITE);
        }
        if let Some(p) = self.pointer.filter(|_| self.pointer_marker) {
            canvas.fill_edges(
                p.x - MARKER,
                p.y - 1.0,
                p.x + MARKER,
                p.y + 1.0,
                Color::WHITE,
            );
            canvas.fill_edges(
                p.x - 1.0,
                p.y - MARKER,
                p.x + 1.0,
                p.y + MARKER,
                Color::WHITE,
            );
        }
        state
    }

    fn track_pointer(&mut self, effect: &InputEffect) -> bool {
        let next = match effect {
            InputEffect::PointerEnter { pos } | InputEffect::PointerMotion { pos } => Some(*pos),
            InputEffect::PointerLeave => None,
            _ => return false,
        };
        let changed = next != self.pointer;
        self.pointer = next;
        changed
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::Rect;

    use super::*;
    use crate::input::{AxisFrame, AxisSource};

    fn options(kinds: &[SceneKind]) -> SceneOptions {
        SceneOptions {
            kinds: kinds.to_vec(),
            ..SceneOptions::default()
        }
    }

    fn render(set: &mut SceneSet, frame_id: u64, time: Timestamp) -> (Vec<u8>, FrameState) {
        let mut data = vec![0; 400 * 300 * 4];
        let mut c = Canvas::new(&mut data, Size::new(400, 300), 1.0);
        let ctx = DrawContext {
            window: Size::new(400, 300),
            frame_id,
            time,
        };
        let state = set.draw(&mut c, &ctx);
        (data, state)
    }

    #[test]
    fn static_scenes_draw_the_same_frame_twice() {
        let mut set = SceneSet::new(&options(&[
            SceneKind::Hover,
            SceneKind::Click,
            SceneKind::Scroll,
            SceneKind::Type,
            SceneKind::Resize,
        ]));
        assert!(!set.animating());
        let (a, _) = render(&mut set, 1, Timestamp(0));
        let (b, _) = render(&mut set, 2, Timestamp(1_000_000_000));
        assert_eq!(a, b);
    }

    #[test]
    fn animation_changes_every_frame() {
        let mut set = SceneSet::new(&options(&[SceneKind::Animate]));
        assert!(set.animating());
        let (a, s1) = render(&mut set, 1, Timestamp(0));
        let (b, s2) = render(&mut set, 2, Timestamp(16_666_667));
        assert_ne!(a, b);
        assert_eq!(s1.bar_x, Some(0.0));
        assert!((s2.bar_x.unwrap() - 10.0).abs() < 1e-3);
    }

    #[test]
    fn hover_input_changes_the_picture() {
        let mut set = SceneSet::new(&options(&[SceneKind::Hover]));
        let window = Size::new(400, 300);
        let (a, s) = render(&mut set, 1, Timestamp(0));
        assert_eq!(s.hover, Some(false));
        let inside = InputEffect::PointerMotion {
            pos: Rect::<Logical>::new(100, 100, 200, 150).center(),
        };
        assert!(set.apply(&inside, window));
        assert!(!set.apply(&inside, window));
        let (b, s) = render(&mut set, 2, Timestamp(0));
        assert_eq!(s.hover, Some(true));
        assert_ne!(a, b);
    }

    #[test]
    fn scroll_and_marker_report_changes() {
        let mut o = options(&[SceneKind::Scroll]);
        o.pointer_marker = true;
        let mut set = SceneSet::new(&o);
        let window = Size::new(400, 300);
        let axis = InputEffect::Axis(AxisFrame {
            source: Some(AxisSource::Finger),
            dy: 12.0,
            ..AxisFrame::default()
        });
        assert!(set.apply(&axis, window));
        assert!(set.apply(
            &InputEffect::PointerMotion {
                pos: Point::new(5.0, 5.0)
            },
            window
        ));
        let (_, s) = render(&mut set, 1, Timestamp(0));
        assert_eq!(s.scroll, Some([0.0, 12.0]));
    }

    #[test]
    fn frame_id_overlay_changes_with_the_id() {
        let mut o = options(&[SceneKind::Idle]);
        o.show_frame_id = true;
        let mut set = SceneSet::new(&o);
        assert!(set.idle());
        let (a, _) = render(&mut set, 1, Timestamp(0));
        let (b, _) = render(&mut set, 2, Timestamp(0));
        assert_ne!(a, b);
    }
}
