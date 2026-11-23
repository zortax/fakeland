//! The command line of the layer client.

use clap::{Parser, ValueEnum};

/// What the client shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    /// One layer surface, with an optional popup.
    Layer,
    /// A session lock with one lock surface per output.
    Lock,
    /// One xdg toplevel whose app id and title are the namespace, with an optional popup.
    Toplevel,
}

/// The layer of the layer surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum LayerArg {
    /// Below every other surface.
    Background,
    /// Below the toplevels.
    Bottom,
    /// Above the toplevels.
    Top,
    /// Above every other surface.
    Overlay,
}

/// The keyboard interactivity of the layer surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum KeyboardArg {
    /// No keyboard focus.
    None,
    /// Exclusive keyboard focus.
    Exclusive,
    /// Keyboard focus on demand.
    OnDemand,
}

/// A rectangle in surface-local logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RectArg {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

/// An RGBA color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorArg(pub [u8; 4]);

/// Wayland client that shows a layer-shell surface or locks the session, and logs every
/// configure and input event as JSON lines on stdout.
#[derive(Clone, Debug, Parser)]
#[command(name = "fakeland-layerclient", version)]
pub struct Args {
    /// What the client shows.
    #[arg(long, value_enum, default_value_t = Mode::Layer)]
    pub mode: Mode,
    /// The layer surface namespace.
    #[arg(long, default_value = "fakeland-layer")]
    pub namespace: String,
    /// The layer.
    #[arg(long, value_enum, default_value_t = LayerArg::Top)]
    pub layer: LayerArg,
    /// The anchored edges, comma-separated: top, bottom, left, right.
    #[arg(long, value_delimiter = ',')]
    pub anchor: Vec<String>,
    /// The requested size as WxH. 0 stretches between two anchored edges.
    #[arg(long, default_value = "200x100", value_parser = parse_size)]
    pub size: (u32, u32),
    /// The margins as TOP,RIGHT,BOTTOM,LEFT.
    #[arg(long, default_value = "0,0,0,0", value_parser = parse_margin)]
    pub margin: [i32; 4],
    /// The exclusive zone.
    #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
    pub exclusive_zone: i32,
    /// The keyboard interactivity.
    #[arg(long, value_enum, default_value_t = KeyboardArg::None)]
    pub keyboard: KeyboardArg,
    /// The input region as X,Y,W,H. The whole surface takes input without it.
    #[arg(long, value_parser = parse_rect)]
    pub input_region: Option<RectArg>,
    /// The fill color as #RRGGBB or #RRGGBBAA.
    #[arg(long, default_value = "#3060c0", value_parser = parse_color)]
    pub color: ColorArg,
    /// The output index in registry order. The compositor picks one without it.
    #[arg(long)]
    pub output: Option<usize>,
    /// A popup at X,Y,W,H, relative to the layer surface or the toplevel.
    #[arg(long, value_parser = parse_rect)]
    pub popup: Option<RectArg>,
    /// Open the popup at a button press on the layer surface or the toplevel, and grab the
    /// seat with it. The popup closes at `popup_done`, and the next press opens it again.
    #[arg(long)]
    pub popup_grab: bool,
    /// The popup fill color.
    #[arg(long, default_value = "#e06020", value_parser = parse_color)]
    pub popup_color: ColorArg,
    /// Unmap the layer surface at a button press on it, and map it again at once in the popup
    /// color.
    #[arg(long)]
    pub remap_on_press: bool,
    /// Unlock the session at the first button or key press.
    #[arg(long)]
    pub unlock_on_input: bool,
    /// Unlock and lock again in one flush at the first button or key press. The old lock
    /// surfaces stay. With `--unlock-on-input` the next press unlocks.
    #[arg(long)]
    pub relock_on_input: bool,
    /// Unlock the session this many milliseconds after the first `locked`.
    #[arg(long)]
    pub unlock_after_ms: Option<u64>,
    /// Lock the session again this many milliseconds after the first `locked`, with new lock
    /// surfaces and without an unlock of the first lock.
    #[arg(long)]
    pub lock_again_after_ms: Option<u64>,
    /// Exit after this many seconds.
    #[arg(long, default_value_t = 30)]
    pub exit_after: u64,
}

fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s.split_once('x').ok_or("expected WxH")?;
    Ok((
        w.parse().map_err(|_| "bad width")?,
        h.parse().map_err(|_| "bad height")?,
    ))
}

fn parse_ints(s: &str) -> Result<Vec<i32>, String> {
    s.split(',')
        .map(|v| v.trim().parse::<i32>().map_err(|e| e.to_string()))
        .collect()
}

fn parse_margin(s: &str) -> Result<[i32; 4], String> {
    parse_ints(s)?
        .try_into()
        .map_err(|_| "expected TOP,RIGHT,BOTTOM,LEFT".to_owned())
}

fn parse_rect(s: &str) -> Result<RectArg, String> {
    match parse_ints(s)?.as_slice() {
        [x, y, w, h] if *w > 0 && *h > 0 => Ok(RectArg {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
        }),
        _ => Err("expected X,Y,W,H with a positive size".to_owned()),
    }
}

fn parse_color(s: &str) -> Result<ColorArg, String> {
    let hex = s.trim_start_matches('#');
    let byte = |i: usize| {
        u8::from_str_radix(hex.get(i..i + 2).ok_or("short color")?, 16).map_err(|e| e.to_string())
    };
    match hex.len() {
        6 => Ok(ColorArg([byte(0)?, byte(2)?, byte(4)?, 255])),
        8 => Ok(ColorArg([byte(0)?, byte(2)?, byte(4)?, byte(6)?])),
        _ => Err("expected #RRGGBB or #RRGGBBAA".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_parse() {
        assert_eq!(parse_size("0x32"), Ok((0, 32)));
        assert_eq!(parse_margin("1,2,3,4"), Ok([1, 2, 3, 4]));
        assert!(parse_rect("0,0,0,5").is_err());
        assert_eq!(parse_color("#102030"), Ok(ColorArg([16, 32, 48, 255])));
        assert_eq!(parse_color("10203040"), Ok(ColorArg([16, 32, 48, 64])));
    }
}
