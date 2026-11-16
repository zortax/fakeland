//! Conversions from the smithay layer-shell state to the public layer description.

use smithay::desktop::LayerSurface;
use smithay::wayland::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

use crate::shell::{KeyboardMode, LayerInfo, ShellLayer};

/// Returns the public layer of a smithay layer.
pub fn shell_layer(layer: Layer) -> ShellLayer {
    match layer {
        Layer::Background => ShellLayer::Background,
        Layer::Bottom => ShellLayer::Bottom,
        Layer::Top => ShellLayer::Top,
        Layer::Overlay => ShellLayer::Overlay,
    }
}

/// Returns the public keyboard mode of a smithay keyboard interactivity.
pub fn keyboard_mode(k: KeyboardInteractivity) -> KeyboardMode {
    match k {
        KeyboardInteractivity::None => KeyboardMode::None,
        KeyboardInteractivity::Exclusive => KeyboardMode::Exclusive,
        KeyboardInteractivity::OnDemand => KeyboardMode::OnDemand,
    }
}

/// Returns the names of the anchored edges.
pub fn anchor_names(anchor: Anchor) -> Vec<String> {
    [
        (Anchor::TOP, "top"),
        (Anchor::BOTTOM, "bottom"),
        (Anchor::LEFT, "left"),
        (Anchor::RIGHT, "right"),
    ]
    .into_iter()
    .filter(|(edge, _)| anchor.contains(*edge))
    .map(|(_, name)| name.to_owned())
    .collect()
}

/// Returns the public description of the current state of a layer surface.
pub fn layer_info(layer: &LayerSurface) -> LayerInfo {
    let state = layer.cached_state();
    LayerInfo {
        namespace: layer.namespace().to_owned(),
        layer: shell_layer(state.layer),
        keyboard: keyboard_mode(state.keyboard_interactivity),
        exclusive_zone: state.exclusive_zone.into(),
        anchor: anchor_names(state.anchor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_name_each_edge() {
        assert_eq!(anchor_names(Anchor::empty()), Vec::<String>::new());
        assert_eq!(
            anchor_names(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT),
            vec!["top", "left", "right"]
        );
    }

    #[test]
    fn layers_keep_their_order() {
        let order: Vec<ShellLayer> = [Layer::Background, Layer::Bottom, Layer::Top, Layer::Overlay]
            .into_iter()
            .map(shell_layer)
            .collect();
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(order, sorted);
        assert_eq!(
            keyboard_mode(KeyboardInteractivity::OnDemand),
            KeyboardMode::OnDemand
        );
    }
}
