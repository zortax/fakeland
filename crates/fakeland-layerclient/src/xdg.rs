//! The xdg shell: a toplevel with the requested input region, popups of the layer surface or the
//! toplevel, their configures, and pings.

use serde_json::json;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::xdg_popup::{self, XdgPopup};
use wayland_protocols::xdg::shell::client::xdg_positioner::{Anchor, Gravity, XdgPositioner};
use wayland_protocols::xdg::shell::client::xdg_surface::{self, XdgSurface};
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self, XdgToplevel};
use wayland_protocols::xdg::shell::client::xdg_wm_base::{self, XdgWmBase};

use crate::app::{App, Popup, Toplevel};
use crate::log;

impl App {
    /// Sets the requested input region on a surface.
    pub fn apply_input_region(&self, surface: &WlSurface, qh: &QueueHandle<Self>) {
        if let Some(r) = self.args.input_region {
            let region = self.compositor.create_region(qh, ());
            region.add(r.x, r.y, r.w, r.h);
            surface.set_input_region(Some(&region));
            region.destroy();
        }
    }

    /// Creates the toplevel and makes its initial commit. Returns false without the xdg shell.
    pub fn create_toplevel(&mut self, qh: &QueueHandle<Self>) -> bool {
        let Some(wm) = &self.wm_base else {
            return false;
        };
        let surface = self.compositor.create_surface(qh, ());
        let xdg = wm.get_xdg_surface(&surface, qh, ());
        let toplevel = xdg.get_toplevel(qh, ());
        toplevel.set_app_id(self.args.namespace.clone());
        toplevel.set_title(self.args.namespace.clone());
        self.apply_input_region(&surface, qh);
        surface.commit();
        self.toplevel = Some(Toplevel {
            surface,
            xdg,
            toplevel,
            size: (0, 0),
        });
        true
    }

    /// Creates the popup with a parent xdg surface, or with none and a role-specific `assign`.
    pub fn create_popup(
        &mut self,
        parent: Option<&XdgSurface>,
        qh: &QueueHandle<Self>,
        assign: impl FnOnce(&XdgPopup),
    ) {
        let (Some(rect), Some(wm)) = (self.args.popup, &self.wm_base) else {
            return;
        };
        let positioner: XdgPositioner = wm.create_positioner(qh, ());
        positioner.set_size(rect.w, rect.h);
        positioner.set_anchor_rect(rect.x, rect.y, 1, 1);
        positioner.set_anchor(Anchor::TopLeft);
        positioner.set_gravity(Gravity::BottomRight);
        let surface = self.compositor.create_surface(qh, ());
        let xdg = wm.get_xdg_surface(&surface, qh, ());
        let popup = xdg.get_popup(parent, &positioner, qh, ());
        assign(&popup);
        positioner.destroy();
        surface.commit();
        self.popup = Some(Popup {
            surface,
            xdg,
            popup,
        });
    }

    /// Draws the toplevel at its configured size, and creates the popup after the first draw.
    fn draw_toplevel(&mut self, qh: &QueueHandle<Self>) {
        let Some(t) = &self.toplevel else {
            return;
        };
        let w = if t.size.0 > 0 {
            t.size.0 as u32
        } else {
            self.args.size.0
        };
        let h = if t.size.1 > 0 {
            t.size.1 as u32
        } else {
            self.args.size.1
        };
        if let Err(e) = self.draw(&t.surface, w.max(1), h.max(1), self.args.color, qh) {
            eprintln!("fakeland-layerclient: {e}");
        }
        if self.popup.is_none() {
            let parent = t.xdg.clone();
            self.create_popup(Some(&parent), qh, |_| {});
        }
    }
}

impl Dispatch<XdgWmBase, ()> for App {
    fn event(
        _: &mut Self,
        wm: &XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm.pong(serial);
        }
    }
}

impl Dispatch<XdgSurface, ()> for App {
    fn event(
        app: &mut Self,
        xdg: &XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let xdg_surface::Event::Configure { serial } = event else {
            return;
        };
        xdg.ack_configure(serial);
        if app.toplevel.as_ref().is_some_and(|t| t.xdg == *xdg) {
            app.draw_toplevel(qh);
            return;
        }
        let (Some(p), Some(rect)) = (&app.popup, app.args.popup) else {
            return;
        };
        if let Err(e) = app.draw(
            &p.surface,
            rect.w as u32,
            rect.h as u32,
            app.args.popup_color,
            qh,
        ) {
            eprintln!("fakeland-layerclient: {e}");
        }
    }
}

impl Dispatch<XdgToplevel, ()> for App {
    fn event(
        app: &mut Self,
        _: &XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                log::event("configure", json!({ "width": width, "height": height }));
                if let Some(t) = &mut app.toplevel {
                    t.size = (width, height);
                }
            }
            xdg_toplevel::Event::Close => {
                log::event("closed", json!({}));
                app.done = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<XdgPopup, ()> for App {
    fn event(
        _: &mut Self,
        _: &XdgPopup,
        event: xdg_popup::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_popup::Event::Configure {
                x,
                y,
                width,
                height,
            } => log::event(
                "popup_configure",
                json!({ "x": x, "y": y, "width": width, "height": height }),
            ),
            xdg_popup::Event::PopupDone => log::event("popup_done", json!({})),
            _ => {}
        }
    }
}
