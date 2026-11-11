//! The render elements of a capture: windows with their subsurfaces and popups, in stacking order,
//! and the cursor on top.

use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{AsRenderElements, Kind};
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer};
use smithay::desktop::{Space, Window};
use smithay::input::pointer::CursorImageStatus;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Logical, Physical, Point, Rectangle, Scale};

smithay::backend::renderer::element::render_elements! {
    /// One element of a capture.
    pub CaptureElement<R> where R: ImportAll + ImportMem;
    /// A client surface.
    Surface=WaylandSurfaceRenderElement<R>,
    /// The built-in cursor image.
    Memory=MemoryRenderBufferRenderElement<R>,
}

/// The cursor to draw.
pub struct CursorScene<'a> {
    /// The pointer position in the global space.
    pub position: Point<f64, Logical>,
    /// The client cursor image.
    pub image: &'a CursorImageStatus,
    /// The hotspot of a client cursor surface.
    pub hotspot: Point<i32, Logical>,
    /// The built-in cursor image.
    pub default: &'a MemoryRenderBuffer,
    /// The hotspot of the built-in cursor image.
    pub default_hotspot: Point<i32, Logical>,
}

/// The windows of one capture.
pub enum WindowScene<'a> {
    /// Every window of the space that overlaps the region.
    Space(&'a Space<Window>),
    /// One window alone.
    Single(&'a Window, Point<i32, Logical>),
}

/// Collects the elements that cover `region` at `scale`, front to back.
pub fn collect<R>(
    renderer: &mut R,
    scene: &WindowScene<'_>,
    region: Rectangle<i32, Logical>,
    scale: f64,
    cursor: Option<&CursorScene<'_>>,
) -> Vec<CaptureElement<R>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    let scale_f = Scale::from(scale);
    let mut out: Vec<CaptureElement<R>> = Vec::new();
    if let Some(cursor) = cursor
        && region.to_f64().contains(cursor.position)
    {
        out.extend(cursor_elements(renderer, cursor, region, scale));
    }
    let mut push_window = |renderer: &mut R, window: &Window, loc: Point<i32, Logical>| {
        let render_loc = loc - window.geometry().loc - region.loc;
        let physical: Point<i32, Physical> = render_loc.to_physical_precise_round(scale_f);
        out.extend(window.render_elements::<CaptureElement<R>>(renderer, physical, scale_f, 1.0));
    };
    match scene {
        WindowScene::Space(space) => {
            let windows: Vec<(Window, Point<i32, Logical>)> = space
                .elements()
                .rev()
                .filter(|w| w.alive())
                .filter(|w| {
                    space
                        .element_bbox(w)
                        .is_some_and(|bbox| bbox.overlaps(region))
                })
                .filter_map(|w| Some((w.clone(), space.element_geometry(w)?.loc)))
                .collect();
            for (window, loc) in windows {
                push_window(renderer, &window, loc);
            }
        }
        WindowScene::Single(window, loc) if window.alive() => push_window(renderer, window, *loc),
        WindowScene::Single(..) => {}
    }
    out
}

fn cursor_elements<R>(
    renderer: &mut R,
    cursor: &CursorScene<'_>,
    region: Rectangle<i32, Logical>,
    scale: f64,
) -> Vec<CaptureElement<R>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    let scale_f = Scale::from(scale);
    let relative = cursor.position - region.loc.to_f64();
    match cursor.image {
        CursorImageStatus::Hidden => Vec::new(),
        CursorImageStatus::Surface(surface) if !surface.is_alive() => Vec::new(),
        CursorImageStatus::Surface(surface) => {
            let loc = (relative - cursor.hotspot.to_f64())
                .to_physical(scale_f)
                .to_i32_round();
            surface_elements(renderer, surface, loc, scale_f)
        }
        CursorImageStatus::Named(_) => {
            let loc = (relative - cursor.default_hotspot.to_f64()).to_physical(scale_f);
            MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                loc,
                cursor.default,
                None,
                None,
                None,
                Kind::Cursor,
            )
            .map(|e| vec![CaptureElement::Memory(e)])
            .unwrap_or_default()
        }
    }
}

fn surface_elements<R>(
    renderer: &mut R,
    surface: &WlSurface,
    loc: Point<i32, Physical>,
    scale: Scale<f64>,
) -> Vec<CaptureElement<R>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    render_elements_from_surface_tree(renderer, surface, loc, scale, 1.0, Kind::Cursor)
}
