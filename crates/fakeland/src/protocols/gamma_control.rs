//! `zwlr_gamma_control_manager_v1`. Each output takes one gamma control at a time. A faked display
//! has no ramps to change, so the compositor reads each table a client sets and records it in the
//! trace. Destroying the control or losing the output returns the output to its own ramps.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;

use fakeland_trace::Event;
use fakeland_types::OutputId;
use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::gamma_control::v1::server::{
    zwlr_gamma_control_manager_v1::{self, ZwlrGammaControlManagerV1},
    zwlr_gamma_control_v1::{self, ZwlrGammaControlV1},
};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New,
};

use crate::server::State;

/// The entries of each ramp of a faked output.
pub const GAMMA_SIZE: u32 = 256;

/// The gamma control global and the control each output holds.
#[derive(Debug, Default)]
pub struct GammaControlState {
    controls: BTreeMap<OutputId, ZwlrGammaControlV1>,
}

impl GammaControlState {
    /// Creates the global.
    pub fn new(dh: &DisplayHandle) -> Self {
        dh.create_global::<State, ZwlrGammaControlManagerV1, _>(1, ());
        Self::default()
    }
}

/// The last entry of the red, green and blue ramps of `table`, which holds three ramps of `size`
/// native-endian entries.
#[must_use]
pub fn ramp_tops(table: &[u8], size: u32) -> Option<[u16; 3]> {
    let size = usize::try_from(size).ok()?;
    if size == 0 || table.len() != size * 6 {
        return None;
    }
    let entry = |ramp: usize| {
        let at = (ramp * size + size - 1) * 2;
        u16::from_ne_bytes([table[at], table[at + 1]])
    };
    Some([entry(0), entry(1), entry(2)])
}

impl State {
    /// Fails the gamma control of output `id`, which goes away.
    pub fn close_output_gamma(&mut self, id: OutputId) {
        if let Some(control) = self.protocols.gamma_control.controls.remove(&id) {
            control.failed();
            self.emit(Event::Gamma {
                output: id,
                top: None,
            });
        }
    }

    /// Ends the control of the output that `control` holds, if it holds one.
    fn release_gamma(&mut self, control: &ZwlrGammaControlV1, failed: bool) {
        let held = self
            .protocols
            .gamma_control
            .controls
            .iter()
            .find(|(_, it)| *it == control)
            .map(|(id, _)| *id);
        let Some(id) = held else {
            return;
        };
        self.protocols.gamma_control.controls.remove(&id);
        if failed {
            control.failed();
        }
        self.emit(Event::Gamma {
            output: id,
            top: None,
        });
    }

    /// Reads the table a client set on `control` and records it.
    fn set_gamma(&mut self, control: &ZwlrGammaControlV1, file: File) {
        let held = self
            .protocols
            .gamma_control
            .controls
            .iter()
            .find(|(_, it)| *it == control)
            .map(|(id, _)| *id);
        let Some(id) = held else {
            return;
        };
        let mut table = Vec::new();
        let expected = u64::from(GAMMA_SIZE) * 6;
        let read = file.take(expected + 1).read_to_end(&mut table);
        match read.ok().and_then(|_| ramp_tops(&table, GAMMA_SIZE)) {
            Some(top) => {
                self.emit(Event::Gamma {
                    output: id,
                    top: Some(top),
                });
            }
            None => self.release_gamma(control, true),
        }
    }
}

impl GlobalDispatch<ZwlrGammaControlManagerV1, ()> for State {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ZwlrGammaControlManagerV1>,
        _data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwlrGammaControlManagerV1, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &ZwlrGammaControlManagerV1,
        request: zwlr_gamma_control_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_gamma_control_manager_v1::Request::GetGammaControl { id, output } => {
                let control = data_init.init(id, ());
                let output = Output::from_resource(&output).and_then(|o| state.outputs.id_of(&o));
                let controls = &mut state.protocols.gamma_control.controls;
                match output {
                    Some(output) if !controls.contains_key(&output) => {
                        control.gamma_size(GAMMA_SIZE);
                        controls.insert(output, control);
                    }
                    _ => control.failed(),
                }
            }
            zwlr_gamma_control_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<ZwlrGammaControlV1, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &ZwlrGammaControlV1,
        request: zwlr_gamma_control_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_gamma_control_v1::Request::SetGamma { fd } => {
                state.set_gamma(resource, File::from(fd));
            }
            zwlr_gamma_control_v1::Request::Destroy => state.release_gamma(resource, false),
            _ => {}
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: smithay::reexports::wayland_server::backend::ClientId,
        resource: &ZwlrGammaControlV1,
        _data: &(),
    ) {
        state.release_gamma(resource, false);
    }
}

#[cfg(test)]
mod tests {
    use super::ramp_tops;

    #[test]
    fn the_tops_are_the_last_entry_of_each_ramp() {
        let size = 4_u32;
        let mut table = Vec::new();
        for top in [65535_u16, 50000, 30000] {
            for entry in [0_u16, 1, 2, top] {
                table.extend_from_slice(&entry.to_ne_bytes());
            }
        }
        assert_eq!(ramp_tops(&table, size), Some([65535, 50000, 30000]));
        assert_eq!(ramp_tops(&table[..10], size), None, "a short table");
        table.push(0);
        assert_eq!(ramp_tops(&table, size), None, "a long table");
    }
}
