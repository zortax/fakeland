//! The computer-use flow of an agent, from Rust. The example looks at a downscaled screenshot,
//! finds the click target by its color, maps the image pixel to logical coordinates, clicks,
//! and waits on the compositor thread until the target shows its toggled color. Then it types
//! text, presses a key combo, sets the clipboard and reads it back.
//!
//! Run with `just example computer_use`, or `cargo run -p fakeland-examples --example
//! computer_use` after `cargo build -p fakeland-testclient`. It runs the reference client with
//! the click and type scenes. The screenshots go to `target/examples/computer_use-before.png` and
//! `target/examples/computer_use-after.png`.

use fakeland::scenario::{Button, PixelCondition, Rgba, TypingCadence, Wait};
use fakeland::{
    CaptureOptions, CaptureTarget, Fakeland, Image, LaunchSpec, OutputConfig, Roi, Screenshot,
    Session, WindowRef,
};
use fakeland_examples::{out_dir, testclient_argv};
use fakeland_scenario::Position;
use fakeland_types::{Dur, Hz, Logical, Nanos, Rect};

/// The click target color after an even number of presses.
const IDLE: Rgba = Rgba::rgb(0x20, 0x40, 0x20);
/// The click target color after an odd number of presses.
const ACTIVE: Rgba = Rgba::rgb(0xc0, 0xc0, 0x30);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = Fakeland::builder()
        .output(
            OutputConfig::new("DP-1")
                .mode(1920, 1080, Hz(60.0))
                .scale(1.5),
        )
        .start()?;
    let argv = testclient_argv(&[
        "--scene",
        "click,type",
        "--click-rect",
        "400,300,160,120",
        "--click-color",
        "#204020",
        "--click-active-color",
        "#c0c030",
    ]);
    let app = session.blocking_launch(LaunchSpec::from_argv(argv).ok_or("empty command")?)?;
    let window = app.blocking_wait_window(Nanos::from_secs(10))?;
    session.blocking_wait(Wait::settled(
        Some(WindowRef::id(window.id)),
        Dur::from_millis(150),
    ))?;
    let g = window.geometry;
    println!(
        "1. window {:?} maps at ({}, {}), {}x{} logical px",
        window.title, g.x, g.y, g.w, g.h
    );

    let before = screenshot(&session, "computer_use-before.png")?;
    let (px, py) = find_color(&before.image, IDLE, 12).ok_or("no click target on screen")?;
    let target = before.to_logical(px, py);
    println!(
        "2. the target is at image pixel ({px:.0}, {py:.0}), logical ({:.0}, {:.0})",
        target.x, target.y
    );

    // The only output sits at the origin, so global logical coordinates are output coordinates.
    let at = Position::output(target.x, target.y);
    let sent = session.blocking_click(at, Button::Left)?;
    let press = sent.first().map(|s| s.time).ok_or("no click sent")?;
    let probe = Rect::<Logical>::new(target.x as i32 - 4, target.y as i32 - 4, 8, 8);
    let toggled = session.blocking_wait(
        Wait::pixels_match(
            Roi::output_rect(None, probe),
            PixelCondition::MeanColor {
                rgba: ACTIVE,
                tolerance: 8,
            },
        )
        .with_timeout(Dur::from_secs(2)),
    )?;
    println!(
        "3. click: the target shows {ACTIVE} {:.1} ms after the press",
        toggled.time.since(press).as_millis_f64()
    );

    let text = "hello, fakeland";
    session.blocking_type_text(text, TypingCadence::wpm(600.0))?;
    session.blocking_combo("shift+1")?;
    session.blocking_key("Return")?;
    session.blocking_wait(Wait::settled(
        Some(WindowRef::id(window.id)),
        Dur::from_millis(150),
    ))?;
    println!("4. typed {text:?}, then shift+1 and Return");

    session.blocking_set_clipboard_text("copied by the agent")?;
    let clip = session.blocking_clipboard_get(Nanos::from_secs(2))?;
    println!("5. clipboard reads back {clip:?}");
    if let Ok(paste) = session.blocking_launch(LaunchSpec::new("wl-paste").arg("--no-newline")) {
        paste.blocking_wait();
        println!("   wl-paste in the session reads {:?}", paste.stdout());
    }

    let after = screenshot(&session, "computer_use-after.png")?;
    let changed = changed_pixels(&before.image, &after.image);
    println!("6. {changed} image pixels differ between the two screenshots");
    session.blocking_shutdown()?;
    Ok(())
}

/// Takes a screenshot of the output at most 960 pixels wide and writes it as PNG.
fn screenshot(session: &Session, name: &str) -> Result<Screenshot, Box<dyn std::error::Error>> {
    let shot = session.blocking_screenshot(
        CaptureTarget::Output { output: None },
        CaptureOptions {
            include_cursor: false,
            max_dim: Some(960),
        },
    )?;
    let path = out_dir().join(name);
    std::fs::write(&path, shot.image.encode_png()?)?;
    println!(
        "   screenshot {}x{} px, 1 px = {:.3} logical px: {}",
        shot.image.width(),
        shot.image.height(),
        shot.scale,
        path.display()
    );
    Ok(shot)
}

/// Returns the center of the pixels within `tolerance` of a color, in image pixels.
fn find_color(image: &Image, color: Rgba, tolerance: u8) -> Option<(f64, f64)> {
    let (mut sx, mut sy, mut n) = (0.0, 0.0, 0u64);
    for y in 0..image.height() {
        for x in 0..image.width() {
            let p = image.pixel(x, y)?;
            if (0..3).all(|c| p[c].abs_diff(color.0[c]) <= tolerance) {
                sx += f64::from(x) + 0.5;
                sy += f64::from(y) + 0.5;
                n += 1;
            }
        }
    }
    (n > 0).then(|| (sx / n as f64, sy / n as f64))
}

/// Counts the pixels whose color differs between two images of the same size.
fn changed_pixels(a: &Image, b: &Image) -> usize {
    let (w, h) = (a.width().min(b.width()), a.height().min(b.height()));
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| a.pixel(x, y) != b.pixel(x, y))
        .count()
}
