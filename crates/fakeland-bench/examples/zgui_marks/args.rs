//! The command line: the zgui example binary, the output, and the hover, click and scroll
//! targets. The zgui examples `counter` and `styled` have built-in targets.

use std::path::{Path, PathBuf};

use fakeland_types::{Logical, Point, Rect};

/// A hover target: the pointer moves to `at` and back to `away`.
#[derive(Clone, Copy, Debug)]
pub struct Hover {
    /// The hovered point, window-relative.
    pub at: Point<Logical>,
    /// The point the alternate repeats move to, window-relative.
    pub away: Point<Logical>,
    /// The region whose pixels the hover changes, window-relative.
    pub roi: Rect<Logical>,
}

/// A click target.
#[derive(Clone, Copy, Debug)]
pub struct Click {
    /// The clicked point, window-relative.
    pub at: Point<Logical>,
    /// The region whose pixels the click changes, window-relative.
    pub roi: Rect<Logical>,
}

/// A scroll target.
#[derive(Clone, Copy, Debug)]
pub struct Scroll {
    /// The point the scroll events go to, window-relative.
    pub at: Point<Logical>,
    /// The region whose content moves, window-relative.
    pub roi: Rect<Logical>,
    /// The glide velocity in logical pixels per second.
    pub velocity: f64,
    /// The glide duration in milliseconds.
    pub millis: u64,
}

/// The parsed command line.
#[derive(Clone, Debug)]
pub struct Args {
    /// The zgui example binary and its arguments.
    pub command: Vec<String>,
    /// The output directory.
    pub out: PathBuf,
    /// The repeats of each probe.
    pub repeats: u32,
    /// The output size in logical pixels.
    pub size: (i32, i32),
    /// The output refresh rate.
    pub hz: f64,
    /// The hover target.
    pub hover: Option<Hover>,
    /// The click target.
    pub click: Option<Click>,
    /// The scroll target.
    pub scroll: Option<Scroll>,
    /// Prints one line per probe repeat.
    pub verbose: bool,
}

/// The usage text.
pub const USAGE: &str = "usage: zgui_marks [--out DIR] [--repeats N] [--size WxH] [--hz HZ] \
[--hover X,Y --away X,Y --hover-roi X,Y,W,H] [--click X,Y --click-roi X,Y,W,H] \
[--scroll X,Y --scroll-roi X,Y,W,H --velocity PX_PER_S --scroll-ms MS] [--verbose] \
-- ZGUI_EXAMPLE [ARGS...]";

impl Args {
    /// Parses the arguments after the program name.
    pub fn parse(raw: &[String]) -> Result<Self, String> {
        let split = raw.iter().position(|a| a == "--").ok_or(USAGE)?;
        let (flags, command) = (&raw[..split], raw[split + 1..].to_vec());
        if command.is_empty() {
            return Err(USAGE.into());
        }
        let mut args = defaults(&command[0]);
        args.command = command;
        let mut it = flags.iter();
        while let Some(flag) = it.next() {
            let mut value = || it.next().cloned().ok_or(format!("{flag} needs a value"));
            match flag.as_str() {
                "--out" => args.out = PathBuf::from(value()?),
                "--repeats" => args.repeats = number(&value()?)?,
                "--size" => args.size = size(&value()?)?,
                "--hz" => args.hz = number(&value()?)?,
                "--hover" => hover(&mut args).at = point(&value()?)?,
                "--away" => hover(&mut args).away = point(&value()?)?,
                "--hover-roi" => hover(&mut args).roi = rect(&value()?)?,
                "--click" => click(&mut args).at = point(&value()?)?,
                "--click-roi" => click(&mut args).roi = rect(&value()?)?,
                "--scroll" => scroll(&mut args).at = point(&value()?)?,
                "--scroll-roi" => scroll(&mut args).roi = rect(&value()?)?,
                "--velocity" => scroll(&mut args).velocity = number(&value()?)?,
                "--scroll-ms" => scroll(&mut args).millis = number(&value()?)?,
                "--verbose" => args.verbose = true,
                other => return Err(format!("unknown flag {other}\n{USAGE}")),
            }
        }
        if args.hover.is_none() && args.click.is_none() && args.scroll.is_none() {
            return Err(format!(
                "no hover, click or scroll target for {}\n{USAGE}",
                args.command[0]
            ));
        }
        Ok(args)
    }
}

/// The built-in targets of a zgui example, by its file name.
fn defaults(binary: &str) -> Args {
    let name = Path::new(binary)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut args = Args {
        command: Vec::new(),
        out: PathBuf::from("target/zgui-marks"),
        repeats: 16,
        size: (1280, 720),
        hz: 60.0,
        hover: None,
        click: None,
        scroll: None,
        verbose: false,
    };
    match name.as_str() {
        // The counter card is centered. The "+" button is at (644, 380) with size 57x42, and
        // the count digits sit above the buttons.
        "counter" => {
            args.hover = Some(Hover {
                at: Point::new(672.0, 401.0),
                away: Point::new(60.0, 60.0),
                roi: Rect::new(644, 380, 57, 42),
            });
            args.click = Some(Click {
                at: Point::new(672.0, 401.0),
                roi: Rect::new(590, 295, 100, 75),
            });
        }
        // The gallery is about 700 px tall, so a 480 px output scrolls it.
        "styled" => {
            args.size = (1280, 480);
            args.scroll = Some(Scroll {
                at: Point::new(640.0, 240.0),
                roi: Rect::new(340, 40, 600, 400),
                velocity: 400.0,
                millis: 400,
            });
        }
        _ => {}
    }
    args
}

fn hover(args: &mut Args) -> &mut Hover {
    args.hover.get_or_insert(Hover {
        at: Point::new(0.0, 0.0),
        away: Point::new(0.0, 0.0),
        roi: Rect::new(0, 0, 1, 1),
    })
}

fn click(args: &mut Args) -> &mut Click {
    args.click.get_or_insert(Click {
        at: Point::new(0.0, 0.0),
        roi: Rect::new(0, 0, 1, 1),
    })
}

fn scroll(args: &mut Args) -> &mut Scroll {
    args.scroll.get_or_insert(Scroll {
        at: Point::new(0.0, 0.0),
        roi: Rect::new(0, 0, 1, 1),
        velocity: 400.0,
        millis: 400,
    })
}

fn number<T: std::str::FromStr>(s: &str) -> Result<T, String> {
    s.parse().map_err(|_| format!("not a number: {s}"))
}

fn numbers(s: &str, n: usize) -> Result<Vec<f64>, String> {
    let v = s
        .split(',')
        .map(number::<f64>)
        .collect::<Result<Vec<_>, _>>()?;
    if v.len() == n {
        Ok(v)
    } else {
        Err(format!("expected {n} comma-separated numbers: {s}"))
    }
}

fn point(s: &str) -> Result<Point<Logical>, String> {
    let v = numbers(s, 2)?;
    Ok(Point::new(v[0], v[1]))
}

fn rect(s: &str) -> Result<Rect<Logical>, String> {
    let v = numbers(s, 4)?;
    Ok(Rect::new(
        v[0] as i32,
        v[1] as i32,
        v[2] as i32,
        v[3] as i32,
    ))
}

fn size(s: &str) -> Result<(i32, i32), String> {
    let (w, h) = s.split_once('x').ok_or(format!("expected WxH: {s}"))?;
    Ok((number(w)?, number(h)?))
}
