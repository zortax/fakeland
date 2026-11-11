//! The injector state: the keymap, held keys and buttons, touch points, open event groups, the
//! wheel remainder of a classic wheel and the motion report clock.

use std::collections::HashMap;

use fakeland_scenario::Key;
use fakeland_trace::InputSource;
use fakeland_types::{Nanos, Timestamp};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};

use super::{KeymapLookup, PendingSent, SentSink};
use crate::config::PointerDevice;
use crate::display::TimerId;

/// A key that is down, with the keys pressed for it.
#[derive(Clone, Debug)]
pub struct HeldKey {
    /// The key as the caller named it.
    pub key: Key,
    /// The xkb keycode.
    pub keycode: u32,
    /// Modifier keycodes pressed for the shift level, in press order.
    pub implied: Vec<u32>,
    /// The key lives on the temporary keymap.
    pub temporary: bool,
}

/// A touch point that is down.
#[derive(Clone, Debug)]
pub struct TouchPoint {
    /// The surface the point went down on, with its global origin.
    pub focus: Option<(WlSurface, Point<f64, Logical>)>,
}

/// The temporary keymap with keysyms the layout lacks.
#[derive(Clone, Debug)]
pub struct TempKeymap {
    /// The extra keysyms with their keycodes.
    pub keys: Vec<(u32, u32)>,
}

/// A motion that waits for the next report slot of the pointer device.
#[derive(Clone)]
pub struct DeferredMotion {
    /// The global target.
    pub point: Point<f64, Logical>,
    /// What injected it.
    pub source: InputSource,
    /// The task that sends it.
    pub timer: TimerId,
    /// The time the task sends it.
    pub at: Timestamp,
    /// The sink of the batch that injected it last. It takes the record when the task sends the
    /// motion.
    pub sink: Option<SentSink>,
}

impl std::fmt::Debug for DeferredMotion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeferredMotion")
            .field("point", &self.point)
            .field("source", &self.source)
            .field("timer", &self.timer)
            .field("at", &self.at)
            .field("sink", &self.sink.is_some())
            .finish()
    }
}

/// The injector state.
#[derive(Debug)]
pub struct InputState {
    /// The base keymap.
    pub keymap: KeymapLookup,
    /// The active temporary keymap.
    pub temp: Option<TempKeymap>,
    /// Keys that are down.
    pub held_keys: Vec<HeldKey>,
    /// Buttons that are down, as evdev codes.
    pub held_buttons: Vec<u32>,
    /// Touch points that are down.
    pub touches: HashMap<u32, TouchPoint>,
    /// Pointer events that wait for `wl_pointer.frame`.
    pub pointer_group: Vec<PendingSent>,
    /// Touch events that wait for `wl_touch.frame`.
    pub touch_group: Vec<PendingSent>,
    /// The axes that scrolled since the last axis stop.
    pub scroll_axes: (bool, bool),
    /// The `value120` remainder of a classic wheel.
    pub wheel_remainder: (i32, i32),
    /// The pointer device.
    pub device: PointerDevice,
    /// The time of the last motion report.
    pub last_motion: Option<Timestamp>,
    /// A motion that waits for its report slot.
    pub deferred: Option<DeferredMotion>,
}

impl InputState {
    /// Builds the state for a keymap and a pointer device.
    pub fn new(keymap: KeymapLookup, device: PointerDevice) -> Self {
        Self {
            keymap,
            temp: None,
            held_keys: Vec::new(),
            held_buttons: Vec::new(),
            touches: HashMap::new(),
            pointer_group: Vec::new(),
            touch_group: Vec::new(),
            scroll_axes: (false, false),
            wheel_remainder: (0, 0),
            device,
            last_motion: None,
            deferred: None,
        }
    }

    /// Returns the motion report period of the pointer device.
    pub fn report_period(&self) -> Option<Nanos> {
        self.device
            .report_rate
            .map(|r| r.period())
            .filter(|p| *p > Nanos::ZERO)
    }
}
