//! Video recording: a 3 s H.264 recording of vkcube at 60 Hz, the frame count against the
//! vblank grid, repeats of an idle client in Y4M, and a PNG sequence of a window. Run with
//! `cargo test -p fakeland -- --ignored`.

mod support;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use fakeland::{
    Event, Fakeland, Image, LaunchSpec, OutputConfig, RecordSpec, RendererChoice, Session,
    VideoFormat, WindowRef,
};
use fakeland_types::{Hz, Nanos};

fn ffprobe_frames(path: &Path) -> (u64, String, u32, u32) {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=nb_read_frames,r_frame_rate,width,height",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .expect("ffprobe");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("ffprobe json");
    let s = &v["streams"][0];
    (
        s["nb_read_frames"].as_str().unwrap().parse().unwrap(),
        s["r_frame_rate"].as_str().unwrap().to_string(),
        s["width"].as_u64().unwrap() as u32,
        s["height"].as_u64().unwrap() as u32,
    )
}

/// Extracts one frame at a time offset as PNG.
fn frame_at(video: &Path, secs: f64, name: &str) -> Image {
    let out = support::out_dir().join(name);
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-ss", &format!("{secs}"), "-i"])
        .arg(video)
        .args(["-frames:v", "1"])
        .arg(&out)
        .status()
        .expect("ffmpeg");
    assert!(status.success());
    eprintln!("frame at {secs}s: {}", out.display());
    Image::decode_png(&std::fs::read(&out).unwrap()).unwrap()
}

fn differing_pixels(a: &Image, b: &Image) -> usize {
    let mut n = 0;
    for y in (0..a.height()).step_by(4) {
        for x in (0..a.width()).step_by(4) {
            let (p, q) = (a.pixel(x, y).unwrap(), b.pixel(x, y).unwrap());
            if (0..3).any(|c| p[c].abs_diff(q[c]) > 24) {
                n += 1;
            }
        }
    }
    n
}

#[test]
#[ignore = "runs vkcube and ffmpeg"]
fn vkcube_three_seconds_at_60_hz() {
    if !support::have("vkcube") || !support::have("ffprobe") {
        eprintln!("vkcube or ffprobe is missing");
        return;
    }
    let _guard = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .renderer(RendererChoice::Gles)
        .start()
        .expect("start");
    let _cube = session
        .blocking_launch(LaunchSpec::new("vkcube").args(["--wsi", "wayland"]))
        .expect("vkcube");
    session
        .blocking_wait_window(WindowRef::Any, Nanos::from_secs(10))
        .expect("window");
    session
        .blocking_wait_frames(WindowRef::Any, 30, Nanos::from_secs(10))
        .expect("frames");
    let path = support::out_dir().join("vkcube.mp4");
    let _ = std::fs::remove_file(&path);
    let capture = session.blocking_begin_capture().unwrap();
    let rec = session
        .blocking_record(RecordSpec::output(&path))
        .expect("record");
    let listed = session.blocking_recordings().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].width, listed[0].height), (1280, 720));
    std::thread::sleep(Duration::from_secs(3));
    let report = rec.blocking_stop().expect("stop");
    let trace = session.blocking_end_capture(capture).unwrap();
    eprintln!("{}", report.to_text());
    assert!(report.ok(), "{:?}", report.error);
    let span = report.stopped.since(report.first_slot);
    let expected = span.as_nanos() / Nanos(16_666_667).as_nanos() + 1;
    assert_eq!(report.frames as i64, expected, "one frame per vblank");
    assert!((175..=185).contains(&report.frames), "{}", report.frames);
    assert!(report.rendered >= 170, "vkcube presents every vblank");
    assert_eq!(report.frames, report.rendered + report.repeated);
    let captures = support::count(&trace, |e| matches!(e, Event::Capture { .. }));
    assert!(captures as u64 >= report.rendered);
    let (frames, rate, w, h) = ffprobe_frames(&path);
    eprintln!("ffprobe: {frames} frames, {rate} fps, {w}x{h}");
    assert_eq!(frames, report.frames);
    assert_eq!((w, h), (1280, 720));
    let (num, den) = rate.split_once('/').unwrap();
    let fps = num.parse::<f64>().unwrap() / den.parse::<f64>().unwrap();
    assert!((fps - 60.0).abs() < 0.01, "{rate}");
    let validity = session.blocking_validity().unwrap();
    eprintln!(
        "wake lateness: max {:.3} ms, late wakes {}",
        validity.max_lateness.as_millis_f64(),
        validity.late_wakes
    );
    let a = frame_at(&path, 0.5, "vkcube-0.5s.png");
    let b = frame_at(&path, 1.5, "vkcube-1.5s.png");
    let c = frame_at(&path, 2.5, "vkcube-2.5s.png");
    assert!(support::distinct_colors(&a) > 20, "the cube is on screen");
    let (ab, bc) = (differing_pixels(&a, &b), differing_pixels(&b, &c));
    eprintln!("differing pixels: {ab} and {bc}");
    assert!(ab > 500 && bc > 500, "the cube rotates");
    session.blocking_shutdown().unwrap();
}

#[test]
#[ignore = "runs the reference client"]
fn idle_window_repeats_in_y4m_and_png() {
    let _guard = support::serial();
    let session: Session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .renderer(RendererChoice::Pixman)
        .start()
        .expect("start");
    let _client = session
        .blocking_launch(LaunchSpec::new(support::testclient()).args(["--scene", "idle"]))
        .expect("client");
    session
        .blocking_wait_settled(
            WindowRef::Any,
            Nanos::from_millis(200),
            Nanos::from_secs(10),
        )
        .expect("settled");
    let y4m = support::out_dir().join("idle.y4m");
    let rec = session
        .blocking_record(
            RecordSpec::window(WindowRef::Any, &y4m)
                .fps(Hz(30.0))
                .max_dim(320),
        )
        .expect("record");
    let pngs = support::out_dir().join("idle-png");
    let _ = std::fs::remove_dir_all(&pngs);
    let rec_png = session
        .blocking_record(RecordSpec::window(WindowRef::Any, &pngs).fps(Hz(10.0)))
        .expect("record png");
    std::thread::sleep(Duration::from_millis(1000));
    let report = rec.blocking_stop().expect("stop");
    let png_report = rec_png.blocking_stop().expect("stop png");
    eprintln!("{}", report.to_text());
    eprintln!("{}", png_report.to_text());
    assert!(report.ok() && png_report.ok());
    assert_eq!(report.format, VideoFormat::Y4m);
    assert_eq!((report.width, report.height), (320, 180));
    assert!((29..=32).contains(&report.frames), "{}", report.frames);
    assert_eq!(report.rendered, 1, "an idle window renders once");
    assert_eq!(report.repeated, report.frames - 1);
    let bytes = std::fs::read(&y4m).unwrap();
    let header = "YUV4MPEG2 W320 H180 F30:1 Ip A1:1 C420jpeg\n";
    assert!(bytes.starts_with(header.as_bytes()));
    let frame = 6 + 320 * 180 * 3 / 2;
    assert_eq!(bytes.len(), header.len() + report.frames as usize * frame);
    if support::have("ffprobe") {
        let (frames, rate, w, h) = ffprobe_frames(&y4m);
        assert_eq!(
            (frames, rate.as_str(), w, h),
            (report.frames, "30/1", 320, 180)
        );
    }
    let files = std::fs::read_dir(&pngs).unwrap().count() as u64;
    assert_eq!(files, png_report.frames);
    assert!((9..=11).contains(&files), "{files}");
    assert_eq!(png_report.format, VideoFormat::PngSequence);
    let first = Image::decode_png(&std::fs::read(pngs.join("000000.png")).unwrap()).unwrap();
    assert_eq!((first.width(), first.height()), (1280, 720));
    assert!(session.blocking_recordings().unwrap().is_empty());
    assert!(
        session
            .blocking_stop_recording(report.id)
            .is_err_and(|e| e.to_string().contains("no such recording"))
    );
    session.blocking_shutdown().unwrap();
}
