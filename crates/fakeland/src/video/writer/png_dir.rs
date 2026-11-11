//! The PNG sequence sink: one numbered file per frame in a directory. A repeat is a hard link to
//! the previous file, or a copy where links fail.

use std::fs;
use std::path::{Path, PathBuf};

use fakeland_vision::ImageView;

use super::Sink;

/// A directory of PNG frames.
pub struct PngDirSink {
    dir: PathBuf,
    next: u64,
    last: Option<PathBuf>,
}

impl PngDirSink {
    /// Creates the directory.
    pub fn open(dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        Ok(Self {
            dir: dir.to_path_buf(),
            next: 0,
            last: None,
        })
    }

    fn path(&self, i: u64) -> PathBuf {
        self.dir.join(format!("{i:06}.png"))
    }
}

impl Sink for PngDirSink {
    fn write(&mut self, frame: &ImageView<'_>, count: u64) -> Result<(), String> {
        if count == 0 {
            return Ok(());
        }
        let bytes = frame.encode_png().map_err(|e| e.to_string())?;
        let first = self.path(self.next);
        fs::write(&first, bytes).map_err(|e| e.to_string())?;
        self.next += 1;
        for _ in 1..count {
            let p = self.path(self.next);
            let _ = fs::remove_file(&p);
            if fs::hard_link(&first, &p).is_err() {
                fs::copy(&first, &p).map_err(|e| e.to_string())?;
            }
            self.next += 1;
        }
        self.last = Some(first);
        Ok(())
    }

    fn finish(self: Box<Self>) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use fakeland_vision::Image;

    use super::*;

    #[test]
    fn repeats_link_to_the_first_file() {
        let dir = std::env::temp_dir().join(format!("fakeland-png-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut sink = PngDirSink::open(&dir).unwrap();
        sink.write(&Image::filled(4, 4, [1, 2, 3, 255]).view(), 3)
            .unwrap();
        sink.write(&Image::filled(4, 4, [9, 9, 9, 255]).view(), 1)
            .unwrap();
        Box::new(sink).finish().unwrap();
        let names: Vec<String> = {
            let mut v: Vec<String> = fs::read_dir(&dir)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            v.sort();
            v
        };
        assert_eq!(
            names,
            ["000000.png", "000001.png", "000002.png", "000003.png"]
        );
        let a = Image::decode_png(&fs::read(dir.join("000002.png")).unwrap()).unwrap();
        assert_eq!(a.pixel(0, 0), Some([1, 2, 3, 255]));
        fs::remove_dir_all(&dir).unwrap();
    }
}
