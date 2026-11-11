//! The list of presets.

use super::spec::preset;
use super::{Preset, PresetInfo};

/// The preset names in listing order.
pub const NAMES: &[&str] = &[
    "pacing",
    "hover-latency",
    "click-latency",
    "key-latency",
    "typing",
    "scroll-glide",
    "fling",
    "drag-follow",
    "resize-storm",
    "startup",
    "popup-open",
    "throughput",
    "idle",
    "refresh-sweep",
    "scale-switch",
];

/// Returns every preset in listing order.
pub fn all() -> Vec<Preset> {
    vec![
        preset::<super::pacing::Pacing>(),
        preset::<super::hover::Hover>(),
        preset::<super::click::Click>(),
        preset::<super::key::KeyPreset>(),
        preset::<super::typing::TypingPreset>(),
        preset::<super::scroll::ScrollPreset>(),
        preset::<super::fling::FlingPreset>(),
        preset::<super::drag::DragPreset>(),
        preset::<super::resize::ResizePreset>(),
        preset::<super::startup::StartupPreset>(),
        preset::<super::popup::PopupPreset>(),
        preset::<super::throughput::ThroughputPreset>(),
        preset::<super::idle::IdlePreset>(),
        preset::<super::refresh::RefreshPreset>(),
        preset::<super::scale::ScalePreset>(),
    ]
}

/// Returns the listing of every preset with its parameter schema and defaults.
pub fn list() -> Vec<PresetInfo> {
    all().iter().map(Preset::info).collect()
}

/// Returns the preset with a name.
pub fn by_name(name: &str) -> Option<Preset> {
    all().into_iter().find(|p| p.name() == name)
}
