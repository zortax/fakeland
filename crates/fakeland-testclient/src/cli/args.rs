//! The command line of the reference client.

use std::path::PathBuf;

use clap::Parser;
use fakeland_types::{Logical, Nanos, Rect, Size};

use super::values::{
    parse_finite, parse_millis, parse_positive, parse_rect, parse_secs, parse_size,
};
use crate::app::{AnimClock, DamageMode};
use crate::draw::Color;
use crate::load::DelayKind;
use crate::pace::PresentMode;
use crate::scene::{SceneKind, ScrollUnit};
use crate::wl::ScaleMode;

/// Reference Wayland client with controllable rendering, pacing and load. It logs ground truth
/// as JSON lines and through fakeland_marks_v1 when the compositor offers it.
///
/// Durations take a unit (ns, us, ms, s, m, h); a bare number is milliseconds, except for
/// --exit-after, where it is seconds.
#[derive(Clone, Debug, Parser)]
#[command(name = "fakeland-testclient", version)]
pub struct Args {
    /// The xdg_toplevel app id.
    #[arg(long, default_value = "fakeland-testclient", help_heading = "Window")]
    pub app_id: String,
    /// The xdg_toplevel title.
    #[arg(long, default_value = "fakeland testclient", help_heading = "Window")]
    pub title: String,
    /// The logical size when the compositor leaves the size to the client.
    #[arg(long, default_value = "640x480", value_parser = parse_size, help_heading = "Window")]
    pub size: Size<Logical>,
    /// How the client scales its buffers.
    #[arg(long, value_enum, default_value_t = ScaleMode::Auto, help_heading = "Window")]
    pub scale_mode: ScaleMode,
    /// The maximum number of wl_shm buffers.
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..=16), help_heading = "Window")]
    pub buffers: u32,
    /// How the client reports damage.
    #[arg(long, value_enum, default_value_t = DamageMode::Exact, help_heading = "Window")]
    pub damage: DamageMode,
    /// The background color.
    #[arg(long, default_value = "#202020", help_heading = "Window")]
    pub background: Color,

    /// When the client commits frames.
    #[arg(long, value_enum, default_value_t = PresentMode::Callback, help_heading = "Presentation")]
    pub mode: PresentMode,
    /// The commit rate in Hz for --mode fixed-rate.
    #[arg(long, value_parser = parse_positive, help_heading = "Presentation")]
    pub rate: Option<f64>,
    /// Request frame callbacks in the unthrottled and fixed-rate modes too.
    #[arg(long, help_heading = "Presentation")]
    pub request_callbacks: bool,
    /// Request no wp_presentation feedback.
    #[arg(long, help_heading = "Presentation")]
    pub no_feedback: bool,
    /// Wait for and set a wp_fifo_v1 barrier on every commit.
    #[arg(long, help_heading = "Presentation")]
    pub fifo: bool,
    /// Set a wp_commit_timer_v1 target this long after each commit.
    #[arg(long, value_parser = parse_millis, help_heading = "Presentation")]
    pub target_delay: Option<Nanos>,

    /// The scenes, comma-separated or repeated.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "hover",
        help_heading = "Scenes"
    )]
    pub scene: Vec<SceneKind>,
    /// The hover target as X,Y,W,H in logical pixels.
    #[arg(long, default_value = "100,100,200,150", value_parser = parse_rect, help_heading = "Scenes")]
    pub hover_rect: Rect<Logical>,
    /// The hover target color while the pointer is outside.
    #[arg(long, default_value = "#3060c0", help_heading = "Scenes")]
    pub hover_color: Color,
    /// The hover target color while the pointer is inside.
    #[arg(long, default_value = "#e06020", help_heading = "Scenes")]
    pub hover_active_color: Color,
    /// The click target as X,Y,W,H; the whole window without it.
    #[arg(long, value_parser = parse_rect, help_heading = "Scenes")]
    pub click_rect: Option<Rect<Logical>>,
    /// The click target color after an even number of presses.
    #[arg(long, default_value = "#204020", help_heading = "Scenes")]
    pub click_color: Color,
    /// The click target color after an odd number of presses.
    #[arg(long, default_value = "#c0c030", help_heading = "Scenes")]
    pub click_active_color: Color,
    /// How scroll frames become a content offset.
    #[arg(long, value_enum, default_value_t = ScrollUnit::Pixels, help_heading = "Scenes")]
    pub scroll_unit: ScrollUnit,
    /// Logical pixels per wheel detent for --scroll-unit value120.
    #[arg(long, default_value_t = 15.0, value_parser = parse_positive, help_heading = "Scenes")]
    pub wheel_step: f64,
    /// The factor from scroll amount to content offset.
    #[arg(long, default_value_t = 1.0, value_parser = parse_finite, help_heading = "Scenes")]
    pub scroll_factor: f64,
    /// The logical stripe height of the scroll list.
    #[arg(long, default_value_t = 40.0, value_parser = parse_positive, help_heading = "Scenes")]
    pub stripe_height: f64,
    /// The bar speed of the animate scene in logical pixels per second.
    #[arg(long, default_value_t = 600.0, value_parser = parse_finite, help_heading = "Scenes")]
    pub speed: f64,
    /// The logical bar width of the animate scene.
    #[arg(long, default_value_t = 40.0, value_parser = parse_positive, help_heading = "Scenes")]
    pub bar_width: f64,
    /// The bar color of the animate scene.
    #[arg(long, default_value = "#f0f0f0", help_heading = "Scenes")]
    pub bar_color: Color,
    /// Which time drives the animation.
    #[arg(long, value_enum, default_value_t = AnimClock::Render, help_heading = "Scenes")]
    pub anim_clock: AnimClock,
    /// Draw a marker at the pointer position.
    #[arg(long, help_heading = "Scenes")]
    pub pointer_marker: bool,
    /// Draw the frame id in the bottom-left corner.
    #[arg(long, help_heading = "Scenes")]
    pub show_frame_id: bool,
    /// Redraw after every input event, also when the picture stays the same.
    #[arg(long, help_heading = "Scenes")]
    pub redraw_on_input: bool,

    /// The minimum time from render begin to commit of every frame.
    #[arg(long, default_value = "0", value_parser = parse_millis, help_heading = "Load")]
    pub render_delay: Nanos,
    /// How the render delay is waited out.
    #[arg(long, value_enum, default_value_t = DelayKind::Spin, help_heading = "Load")]
    pub delay_kind: DelayKind,
    /// Every frame whose id is a multiple of this number gets --jank on top.
    #[arg(long, default_value_t = 0, help_heading = "Load")]
    pub jank_every: u64,
    /// The extra minimum render time of a janky frame.
    #[arg(long, default_value = "0", value_parser = parse_millis, help_heading = "Load")]
    pub jank: Nanos,
    /// CPU work per frame after drawing.
    #[arg(long, default_value = "0", value_parser = parse_millis, help_heading = "Load")]
    pub cpu_burn: Nanos,
    /// A sleep before the client connects.
    #[arg(long, default_value = "0", value_parser = parse_millis, help_heading = "Load")]
    pub startup_delay: Nanos,
    /// A wait between the first configure ack and the first frame.
    #[arg(long, default_value = "0", value_parser = parse_millis, help_heading = "Load")]
    pub first_frame_delay: Nanos,
    /// A delay from receiving an input event to applying it.
    #[arg(long, default_value = "0", value_parser = parse_millis, help_heading = "Load")]
    pub input_delay: Nanos,
    /// A delay from receiving a configure to acking it.
    #[arg(long, default_value = "0", value_parser = parse_millis, help_heading = "Load")]
    pub resize_delay: Nanos,

    /// Write the JSON lines event log to a file, or to stdout for `-`.
    #[arg(long, value_name = "PATH", help_heading = "Ground truth")]
    pub log_json: Option<PathBuf>,
    /// Ignore fakeland_marks_v1 even when the compositor offers it.
    #[arg(long, help_heading = "Ground truth")]
    pub no_marks: bool,
    /// Exit after this time from process start; a bare number is seconds.
    #[arg(long, value_parser = parse_secs, help_heading = "Exit")]
    pub exit_after: Option<Nanos>,
    /// Exit after this many frames are committed and their feedback arrived.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..), help_heading = "Exit")]
    pub exit_after_frames: Option<u64>,
}
