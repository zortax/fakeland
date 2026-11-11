//! Screenshots: capture, PNG encoding, files and inline base64.

use std::path::Path;

use base64::Engine;
use fakeland::{CaptureOptions, Screenshot};

use super::{Controller, OpError};
use crate::methods::{ScreenshotParams, ShotInfo};

/// Encodes a screenshot as PNG bytes.
pub fn encode_png(shot: &Screenshot) -> Result<Vec<u8>, OpError> {
    shot.image
        .encode_png()
        .map_err(|e| OpError::Failed(format!("png encoding failed: {e}")))
}

/// Writes PNG bytes to a file and creates its directory.
pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), OpError> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| OpError::io(dir, e))?;
    }
    std::fs::write(path, bytes).map_err(|e| OpError::io(path, e))
}

/// Builds the public view of a screenshot without its PNG.
pub fn shot_meta(shot: &Screenshot) -> ShotInfo {
    ShotInfo {
        width: shot.image.width(),
        height: shot.image.height(),
        region: shot.region,
        scale: shot.scale,
        output: shot.output,
        window: shot.window,
        time: shot.time,
        path: None,
        png_base64: None,
    }
}

/// Builds the public view of a screenshot. It writes the PNG to `path` and inlines it when asked.
pub fn shot_info(
    shot: &Screenshot,
    path: Option<&Path>,
    inline: bool,
) -> Result<ShotInfo, OpError> {
    let png = if path.is_some() || inline {
        Some(encode_png(shot)?)
    } else {
        None
    };
    if let (Some(path), Some(png)) = (path, png.as_deref()) {
        write_file(path, png)?;
    }
    Ok(ShotInfo {
        path: path.map(Path::to_path_buf),
        png_base64: png
            .filter(|_| inline)
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b)),
        ..shot_meta(shot)
    })
}

impl Controller {
    /// Takes a screenshot and returns the image with its info.
    pub async fn capture(&self, p: &ScreenshotParams) -> Result<Screenshot, OpError> {
        let options = CaptureOptions {
            include_cursor: p.include_cursor,
            max_dim: p.max_dim,
        };
        Ok(self.session().screenshot(p.target.clone(), options).await?)
    }

    /// Takes a screenshot, writes or inlines its PNG and returns its info.
    pub async fn screenshot(&self, p: ScreenshotParams) -> Result<ShotInfo, OpError> {
        if let Some(path) = &p.path
            && !path.is_absolute()
        {
            return Err(OpError::InvalidParams(format!(
                "the screenshot path must be absolute: {}",
                path.display()
            )));
        }
        let shot = self.capture(&p).await?;
        shot_info(&shot, p.path.as_deref(), p.inline)
    }
}
