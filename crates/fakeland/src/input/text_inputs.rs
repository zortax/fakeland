//! The text inputs of every client. The compositor acts as the input method: a text input of the
//! client with keyboard focus gets `enter`, becomes active when the client enables it, and
//! receives committed text with `done` and the serial of the last client commit.

use smithay::reexports::wayland_protocols::wp::text_input::zv3::server::zwp_text_input_v3::ZwpTextInputV3;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

/// One `zwp_text_input_v3` object.
#[derive(Debug, Clone)]
pub struct TextInputInstance {
    /// The object.
    pub resource: ZwpTextInputV3,
    /// The enable state of the next commit.
    pub pending_enable: Option<bool>,
    /// The client enabled the text input.
    pub enabled: bool,
    /// The count of client commits, which is the serial of `done`.
    pub commits: u32,
    /// The surface of the last `enter`.
    pub entered: Option<WlSurface>,
}

/// Every text input and the text focus.
#[derive(Debug, Default)]
pub struct TextInputs {
    list: Vec<TextInputInstance>,
    focus: Option<WlSurface>,
}

impl TextInputs {
    /// Adds a text input and enters it when its client has the focus.
    pub fn add(&mut self, resource: ZwpTextInputV3) {
        let mut instance = TextInputInstance {
            resource,
            pending_enable: None,
            enabled: false,
            commits: 0,
            entered: None,
        };
        if let Some(focus) = self.focus.clone()
            && focus.is_alive()
            && focus.id().same_client_as(&instance.resource.id())
        {
            instance.resource.enter(&focus);
            instance.entered = Some(focus);
        }
        self.list.push(instance);
    }

    /// Removes a text input.
    pub fn remove(&mut self, resource: &ZwpTextInputV3) {
        self.list.retain(|i| i.resource != *resource);
    }

    /// Returns a text input for changes.
    pub fn get_mut(&mut self, resource: &ZwpTextInputV3) -> Option<&mut TextInputInstance> {
        self.list.iter_mut().find(|i| i.resource == *resource)
    }

    /// Moves the text focus: text inputs of the old surface get `leave` and stop being active,
    /// text inputs of the client of the new surface get `enter`.
    pub fn set_focus(&mut self, surface: Option<&WlSurface>) {
        if self.focus.as_ref() == surface {
            return;
        }
        for i in &mut self.list {
            if let Some(old) = i.entered.take() {
                if old.is_alive() && i.resource.is_alive() {
                    i.resource.leave(&old);
                }
                i.enabled = false;
            }
        }
        self.focus = surface.cloned();
        let Some(new) = surface.filter(|s| s.is_alive()) else {
            return;
        };
        for i in &mut self.list {
            if i.resource.is_alive() && new.id().same_client_as(&i.resource.id()) {
                i.resource.enter(new);
                i.entered = Some(new.clone());
            }
        }
    }

    /// Returns the enabled text input of the focused surface, with that surface.
    pub fn active(&self) -> Option<(&TextInputInstance, &WlSurface)> {
        let focus = self.focus.as_ref().filter(|s| s.is_alive())?;
        self.list
            .iter()
            .find(|i| i.enabled && i.entered.as_ref() == Some(focus) && i.resource.is_alive())
            .map(|i| (i, focus))
    }

    /// Commits text on the active text input. Returns the surface that receives it.
    pub fn commit_string(&self, text: &str) -> Option<WlSurface> {
        let (instance, surface) = self.active()?;
        instance.resource.commit_string(Some(text.to_string()));
        instance.resource.done(instance.commits);
        Some(surface.clone())
    }
}
