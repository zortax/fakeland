//! The writer thread: it opens the sink for the format, fits each frame onto the canvas, writes
//! it with its repeats, and reports the frame count and the first error at the end.

use std::path::Path;
use std::sync::mpsc::Receiver;
use std::thread::{self, JoinHandle};

use fakeland_vision::Image;

use super::ffmpeg::FfmpegSink;
use super::png_dir::PngDirSink;
use super::queue::{WriterMsg, queue_frames};
use super::y4m_file::Y4mSink;
use super::{FrameQueue, Sink};
use crate::video::canvas::fit;
use crate::video::{Canvas, FrameRate, RecordError, VideoFormat};

/// What the writer did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriterOutcome {
    /// Frames in the output.
    pub frames: u64,
    /// The first error.
    pub error: Option<String>,
}

/// The writer thread.
#[derive(Debug)]
pub struct WriterThread {
    handle: JoinHandle<WriterOutcome>,
}

impl WriterThread {
    /// Waits for the writer to finish after the queue sent its end.
    pub fn join(self) -> WriterOutcome {
        self.handle.join().unwrap_or_else(|_| WriterOutcome {
            frames: 0,
            error: Some("the writer thread panicked".into()),
        })
    }
}

/// Opens the output and starts the writer thread. Starting ffmpeg forks, so call this off the
/// compositor thread.
pub fn open_writer(
    format: VideoFormat,
    path: &Path,
    canvas: Canvas,
    rate: FrameRate,
) -> Result<(FrameQueue, WriterThread), RecordError> {
    let io = |message: String| RecordError::Io {
        path: path.to_path_buf(),
        message,
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty())
        && format != VideoFormat::PngSequence
    {
        std::fs::create_dir_all(parent).map_err(|e| io(e.to_string()))?;
    }
    let sink: Box<dyn Sink> = match format {
        VideoFormat::Mp4 => Box::new(FfmpegSink::open(path, canvas, rate).map_err(io)?),
        VideoFormat::Y4m => Box::new(Y4mSink::open(path, canvas, rate).map_err(io)?),
        VideoFormat::PngSequence => Box::new(PngDirSink::open(path).map_err(io)?),
    };
    let frame_bytes = canvas.width as usize * canvas.height as usize * 4;
    let (queue, rx) = FrameQueue::new(queue_frames(frame_bytes));
    let handle = thread::Builder::new()
        .name("fakeland-video".into())
        .spawn(move || run(sink, rx, canvas))
        .map_err(|e| io(e.to_string()))?;
    Ok((queue, WriterThread { handle }))
}

fn run(mut sink: Box<dyn Sink>, rx: Receiver<WriterMsg>, canvas: Canvas) -> WriterOutcome {
    let mut out = WriterOutcome::default();
    let mut last: Option<std::sync::Arc<Image>> = None;
    let write = |sink: &mut Box<dyn Sink>, image: &Image, count: u64| {
        if count == 0 {
            return Ok(());
        }
        sink.write(&fit(image, canvas).view(), count)
    };
    for msg in rx {
        let (stale, next, finish) = match msg {
            WriterMsg::Frame {
                stale,
                image,
                count,
            } => (stale, Some((image, count)), false),
            WriterMsg::Finish { stale } => (stale, None, true),
        };
        let result = (|| {
            if let Some(prev) = &last {
                write(&mut sink, prev, stale)?;
                out.frames += stale;
            }
            if let Some((image, count)) = &next {
                write(&mut sink, image, *count)?;
                out.frames += count;
            }
            Ok::<(), String>(())
        })();
        if let Some((image, _)) = next {
            last = Some(image);
        }
        if let Err(e) = result {
            out.error = Some(e);
            return out;
        }
        if finish {
            break;
        }
    }
    if let Err(e) = sink.finish() {
        out.error = Some(e);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn y4m_output_holds_every_slot() {
        let path = std::env::temp_dir().join(format!("fakeland-{}.y4m", std::process::id()));
        let canvas = Canvas::of(8, 4);
        let rate = FrameRate::new(30, 1).unwrap();
        let (mut q, writer) = open_writer(VideoFormat::Y4m, &path, canvas, rate).unwrap();
        q.push(Arc::new(Image::filled(8, 4, [0, 0, 0, 255])), 2);
        q.push(Arc::new(Image::filled(9, 5, [255, 255, 255, 255])), 3);
        q.push(Arc::new(Image::filled(16, 8, [255, 0, 0, 255])), 1);
        q.finish();
        let outcome = writer.join();
        assert_eq!(
            outcome,
            WriterOutcome {
                frames: 6,
                error: None
            }
        );
        let bytes = std::fs::read(&path).unwrap();
        let header = crate::video::y4m::header(8, 4, rate);
        let frame = 6 + 8 * 4 * 3 / 2;
        assert_eq!(bytes.len(), header.len() + 6 * frame);
        let y_of = |i: usize| bytes[header.len() + i * frame + 6];
        assert_eq!((y_of(0), y_of(1), y_of(2), y_of(5)), (16, 16, 235, 82));
        std::fs::remove_file(&path).unwrap();
    }
}
