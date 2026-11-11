//! Configure sequences that wait for their ack. Each configure becomes due after the configure
//! delay. When several are due, the client acks only the newest and the older serials are
//! superseded.

use std::collections::VecDeque;

use fakeland_types::{Logical, Size, Timestamp};

/// One xdg_toplevel.configure plus xdg_surface.configure sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingConfigure {
    /// The xdg_surface serial.
    pub serial: u32,
    /// The suggested logical width; 0 leaves it to the client.
    pub width: i32,
    /// The suggested logical height; 0 leaves it to the client.
    pub height: i32,
    /// The toplevel state names.
    pub states: Vec<String>,
}

impl PendingConfigure {
    /// Returns the size the client uses for this configure. A zero dimension keeps the current
    /// size, or takes the default size before the first configure.
    pub fn size(&self, current: Option<Size<Logical>>, default: Size<Logical>) -> Size<Logical> {
        let base = current.unwrap_or(default);
        Size::new(
            if self.width > 0 { self.width } else { base.w },
            if self.height > 0 { self.height } else { base.h },
        )
    }
}

/// The configures that wait for their ack, oldest first.
#[derive(Debug, Default)]
pub struct ConfigureQueue {
    items: VecDeque<(Timestamp, PendingConfigure)>,
}

impl ConfigureQueue {
    /// Appends a configure that becomes due at `due`.
    pub fn push(&mut self, due: Timestamp, configure: PendingConfigure) {
        self.items.push_back((due, configure));
    }

    /// Returns the due time of the oldest waiting configure.
    pub fn next_due(&self) -> Option<Timestamp> {
        self.items.front().map(|(t, _)| *t)
    }

    /// Removes every configure that is due at `now`. Returns the newest of them and the serials
    /// of the older ones.
    pub fn take_due(&mut self, now: Timestamp) -> Option<(PendingConfigure, Vec<u32>)> {
        let mut newest: Option<PendingConfigure> = None;
        let mut superseded = Vec::new();
        while self.items.front().is_some_and(|(t, _)| *t <= now) {
            let (_, c) = self.items.pop_front()?;
            if let Some(old) = newest.replace(c) {
                superseded.push(old.serial);
            }
        }
        newest.map(|c| (c, superseded))
    }
}

/// Returns the name of an xdg_toplevel state value.
pub fn state_name(value: u32) -> String {
    match value {
        1 => "maximized",
        2 => "fullscreen",
        3 => "resizing",
        4 => "activated",
        5 => "tiled_left",
        6 => "tiled_right",
        7 => "tiled_top",
        8 => "tiled_bottom",
        9 => "suspended",
        10 => "constrained_left",
        11 => "constrained_right",
        12 => "constrained_top",
        13 => "constrained_bottom",
        other => return format!("state_{other}"),
    }
    .to_owned()
}

/// Decodes the native-endian u32 array of xdg_toplevel.configure states.
pub fn decode_states(raw: &[u8]) -> Vec<String> {
    raw.chunks_exact(4)
        .map(|c| state_name(u32::from_ne_bytes([c[0], c[1], c[2], c[3]])))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(serial: u32, w: i32, h: i32) -> PendingConfigure {
        PendingConfigure {
            serial,
            width: w,
            height: h,
            states: Vec::new(),
        }
    }

    #[test]
    fn zero_size_keeps_the_current_size() {
        let default = Size::new(640, 480);
        assert_eq!(cfg(1, 0, 0).size(None, default), default);
        assert_eq!(cfg(1, 800, 0).size(None, default), Size::new(800, 480));
        assert_eq!(
            cfg(1, 0, 0).size(Some(Size::new(100, 50)), default),
            Size::new(100, 50)
        );
        assert_eq!(cfg(1, 300, 200).size(None, default), Size::new(300, 200));
    }

    #[test]
    fn newest_due_configure_wins() {
        let mut q = ConfigureQueue::default();
        q.push(Timestamp(10), cfg(1, 1, 1));
        q.push(Timestamp(20), cfg(2, 2, 2));
        q.push(Timestamp(30), cfg(3, 3, 3));
        assert_eq!(q.take_due(Timestamp(5)), None);
        assert_eq!(q.next_due(), Some(Timestamp(10)));
        let (c, old) = q.take_due(Timestamp(25)).unwrap();
        assert_eq!(c.serial, 2);
        assert_eq!(old, vec![1]);
        let (c, old) = q.take_due(Timestamp(30)).unwrap();
        assert_eq!(c.serial, 3);
        assert!(old.is_empty());
        assert_eq!(q.next_due(), None);
    }

    #[test]
    fn states_decode() {
        let mut raw = Vec::new();
        for v in [1u32, 4, 9, 42] {
            raw.extend_from_slice(&v.to_ne_bytes());
        }
        assert_eq!(
            decode_states(&raw),
            vec!["maximized", "activated", "suspended", "state_42"]
        );
    }
}
