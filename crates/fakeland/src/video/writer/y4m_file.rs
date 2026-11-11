//! The YUV4MPEG2 file sink. A frame converts once, and its repeats write the same planes again.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use fakeland_vision::ImageView;

use super::Sink;
use crate::video::y4m::{FRAME, header, to_i420};
use crate::video::{Canvas, FrameRate};

/// A Y4M file.
pub struct Y4mSink {
    file: BufWriter<File>,
    planes: Vec<u8>,
}

impl Y4mSink {
    /// Creates the file and writes the header.
    pub fn open(path: &Path, canvas: Canvas, rate: FrameRate) -> Result<Self, String> {
        let file = File::create(path).map_err(|e| e.to_string())?;
        let mut file = BufWriter::with_capacity(1 << 20, file);
        file.write_all(header(canvas.width, canvas.height, rate).as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(Self {
            file,
            planes: Vec::new(),
        })
    }
}

impl Sink for Y4mSink {
    fn write(&mut self, frame: &ImageView<'_>, count: u64) -> Result<(), String> {
        to_i420(frame, &mut self.planes);
        for _ in 0..count {
            self.file.write_all(FRAME).map_err(|e| e.to_string())?;
            self.file
                .write_all(&self.planes)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> Result<(), String> {
        self.file.flush().map_err(|e| e.to_string())
    }
}
