//! The render elements of a capture: layer surfaces and windows with their subsurfaces and popups
//! in stacking order, or the lock surfaces alone on a black backdrop while the session is locked,
//! and the cursor on top.

use smithay::backend::renderer::Color32F;
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{AsRenderElements, Id, Kind};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer};
use smithay::desktop::{LayerSurface, Space, Window};
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
    /// The backdrop of a locked session.
    Solid=SolidColorRenderElement,
}

/// One surface tree of a scene with its global origin.
#[derive(Clone, Debug)]
pub struct SceneSurface<T> {
    /// The surface.
    pub surface: T,
    /// The origin of the root surface in the global logical space.
    pub origin: Point<i32, Logical>,
}

/// The lock surfaces of a locked session.
#[derive(Clone, Debug)]
pub struct LockScene {
    /// The lock surfaces.
    pub surfaces: Vec<SceneSurface<WlSurface>>,
    /// The id of the backdrop element.
    pub backdrop: Id,
}

/// The surfaces of a capture around the windows of the space.
#[derive(Clone, Debug, Default)]
pub struct Overlays {
    /// Layer surfaces on the top and overlay layers, topmost first.
    pub above: Vec<SceneSurface<LayerSurface>>,
    /// Layer surfaces on the bottom and background layers, topmost first.
    pub below: Vec<SceneSurface<LayerSurface>>,
    /// The lock surfaces while the session is locked. Nothing else shows then.
    pub lock: Option<LockScene>,
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
    /// Every window of the space and every surface of the overlays that overlaps the region.
    Space(&'a Space<Window>, &'a Overlays),
    /// The surfaces of the overlays alone.
    Overlays(&'a Overlays),
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
    let (space, overlays) = match scene {
        WindowScene::Space(space, overlays) => (Some(*space), Some(*overlays)),
        WindowScene::Overlays(overlays) => (None, Some(*overlays)),
        WindowScene::Single(..) => (None, None),
    };
    match (scene, overlays) {
        (_, Some(overlays)) if overlays.lock.is_some() => {
            if let Some(lock) = &overlays.lock {
                for s in &lock.surfaces {
                    let loc = (s.origin - region.loc).to_physical_precise_round(scale_f);
                    out.extend(surface_tree(renderer, &s.surface, loc, scale_f));
                }
                let size = region.size.to_physical_precise_round(scale_f);
                out.push(CaptureElement::Solid(SolidColorRenderElement::new(
                    lock.backdrop.clone(),
                    Rectangle::from_size(size),
                    CommitCounter::default(),
                    Color32F::BLACK,
                    Kind::Unspecified,
                )));
            }
        }
        (_, Some(overlays)) => {
            push_layers(renderer, &overlays.above, region, scale_f, &mut out);
            let windows: Vec<(Window, Point<i32, Logical>)> = space
                .into_iter()
                .flat_map(|space| space.elements().rev().map(move |w| (space, w)))
                .filter(|(_, w)| w.alive())
                .filter(|(space, w)| {
                    space
                        .element_bbox(w)
                        .is_some_and(|bbox| bbox.overlaps(region))
                })
                .filter_map(|(space, w)| Some((w.clone(), space.element_geometry(w)?.loc)))
                .collect();
            for (window, loc) in windows {
                let render_loc = loc - window.geometry().loc - region.loc;
                let physical: Point<i32, Physical> = render_loc.to_physical_precise_round(scale_f);
                out.extend(
                    window.render_elements::<CaptureElement<R>>(renderer, physical, scale_f, 1.0),
                );
            }
            push_layers(renderer, &overlays.below, region, scale_f, &mut out);
        }
        (WindowScene::Single(window, loc), None) if window.alive() => {
            let render_loc = *loc - window.geometry().loc - region.loc;
            let physical: Point<i32, Physical> = render_loc.to_physical_precise_round(scale_f);
            out.extend(
                window.render_elements::<CaptureElement<R>>(renderer, physical, scale_f, 1.0),
            );
        }
        _ => {}
    }
    out
}

/// Appends the elements of layer surfaces that overlap the region, in the given order.
fn push_layers<R>(
    renderer: &mut R,
    layers: &[SceneSurface<LayerSurface>],
    region: Rectangle<i32, Logical>,
    scale: Scale<f64>,
    out: &mut Vec<CaptureElement<R>>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    for l in layers.iter().filter(|l| l.surface.alive()) {
        let mut bbox = l.surface.bbox_with_popups();
        bbox.loc += l.origin;
        if !bbox.overlaps(region) {
            continue;
        }
        let loc = (l.origin - region.loc).to_physical_precise_round(scale);
        out.extend(
            l.surface
                .render_elements::<CaptureElement<R>>(renderer, loc, scale, 1.0),
        );
    }
}

fn surface_tree<R>(
    renderer: &mut R,
    surface: &WlSurface,
    loc: Point<i32, Physical>,
    scale: Scale<f64>,
) -> Vec<CaptureElement<R>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    if !surface.is_alive() {
        return Vec::new();
    }
    render_elements_from_surface_tree(renderer, surface, loc, scale, 1.0, Kind::Unspecified)
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
