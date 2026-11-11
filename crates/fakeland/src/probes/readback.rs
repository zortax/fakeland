//! Region readbacks for probes: the part of a region of interest that one client buffer covers,
//! read at buffer resolution. Each readback lands in the trace as compositor overhead.

use fakeland_trace::{BufferInfo, Event};
use fakeland_types::{Logical, Nanos, Rect};
use fakeland_vision::Image;
use smithay::backend::renderer::buffer_dimensions;
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical as SLogical, Point};

use super::geometry::BufferPlacement;
use crate::render::read_buffer;
use crate::server::State;

/// A region image with its buffer scale and the readback cost.
#[derive(Clone, Debug)]
pub struct RegionRead {
    /// The pixels.
    pub image: Image,
    /// Buffer pixels per logical unit.
    pub scale: f64,
    /// The compositor time of the readback.
    pub cost: Nanos,
}

impl State {
    /// Returns the applied buffer of a surface and its placement.
    pub fn applied_placement(
        &self,
        surface: &WlSurface,
        origin: Point<i32, SLogical>,
    ) -> Option<(WlBuffer, BufferPlacement)> {
        let (buffer, size) = with_renderer_surface_state(surface, |rs| {
            (rs.buffer().map(|b| (**b).clone()), rs.surface_size())
        })?;
        let buffer = buffer?;
        let size = size?;
        let dims = buffer_dimensions(&buffer)?;
        Some((
            buffer,
            BufferPlacement {
                origin: (origin.x, origin.y),
                logical: (size.w as f64, size.h as f64),
                buffer: (dims.w, dims.h),
            },
        ))
    }

    /// Reads the part of a region that a buffer covers. Returns `None` when the buffer misses the
    /// region or the readback fails.
    pub fn read_region(
        &mut self,
        buffer: &WlBuffer,
        placement: &BufferPlacement,
        roi: Rect<Logical>,
    ) -> Option<RegionRead> {
        let region = placement.region(roi)?;
        let start = self.now();
        let result = read_buffer(&mut self.render, &mut self.shm_reader, buffer, region);
        let end = self.now();
        let cost = end.since(start);
        self.emit_at(
            end,
            Event::CompositorOverhead {
                what: "probe readback".into(),
                duration: cost,
            },
        );
        match result {
            Ok(image) => Some(RegionRead {
                image,
                scale: placement.scale(),
                cost,
            }),
            Err(e) => {
                tracing::debug!("probe readback failed: {e}");
                None
            }
        }
    }
}

/// Returns the placement of a committed buffer from its trace description.
pub fn committed_placement(info: &BufferInfo, origin: Point<i32, SLogical>) -> BufferPlacement {
    let logical = match info.viewport {
        Some(v) => (v.w as f64, v.h as f64),
        None => {
            let s = if info.scale > 0.0 { info.scale } else { 1.0 };
            (info.width as f64 / s, info.height as f64 / s)
        }
    };
    BufferPlacement {
        origin: (origin.x, origin.y),
        logical,
        buffer: (info.width, info.height),
    }
}
