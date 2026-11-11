//! Offers text as the clipboard selection. The compositor serves the bytes itself, and plain
//! UTF-8 text also goes out under the legacy text mime types.

use std::sync::Arc;

use smithay::wayland::selection::data_device::set_data_device_selection;

use crate::server::State;

/// The plain text mime type.
pub const TEXT_MIME: &str = "text/plain;charset=utf-8";

impl State {
    /// Sets the clipboard to bytes that the compositor serves.
    pub fn offer_selection(&mut self, mime: &str, bytes: Arc<[u8]>) {
        let mut mimes = vec![mime.to_string()];
        if mime == TEXT_MIME {
            mimes.extend(
                ["text/plain", "UTF8_STRING", "STRING"]
                    .iter()
                    .map(|s| s.to_string()),
            );
        }
        self.selection = Some(bytes.clone());
        set_data_device_selection(&self.dh, &self.seat, mimes, bytes);
    }
}
