//! Window selection by `WindowRef`: id, app id, title pattern, layer namespace, lock surface,
//! focus or any mapped window.

use fakeland_scenario::WindowRef;
use regex::Regex;
use thiserror::Error;

use super::{WindowInfo, WindowKind};

/// A matcher that cannot run.
#[derive(Debug, Error)]
pub enum MatchError {
    /// The title pattern is not a valid regular expression.
    #[error("bad title pattern {pattern:?}: {source}")]
    Pattern {
        /// The pattern.
        pattern: String,
        /// The parse error.
        source: regex::Error,
    },
}

/// A compiled window matcher.
#[derive(Debug, Clone)]
pub enum WindowMatcher {
    /// Matches a window id.
    Id(fakeland_types::WindowId),
    /// Matches an app id or a layer namespace exactly.
    AppId(String),
    /// Matches a layer namespace exactly.
    Layer(String),
    /// Matches a lock surface.
    Lock,
    /// Matches a title pattern.
    Title(Regex),
    /// Matches the focused window.
    Focused,
    /// Matches any window.
    Any,
}

impl WindowMatcher {
    /// Compiles a window reference.
    pub fn new(r: &WindowRef) -> Result<Self, MatchError> {
        Ok(match r {
            WindowRef::Id { id } => Self::Id(*id),
            WindowRef::AppId { app_id } => Self::AppId(app_id.clone()),
            WindowRef::TitleRegex { pattern } => {
                Self::Title(Regex::new(pattern).map_err(|source| MatchError::Pattern {
                    pattern: pattern.clone(),
                    source,
                })?)
            }
            WindowRef::Layer { namespace } => Self::Layer(namespace.clone()),
            WindowRef::Lock => Self::Lock,
            WindowRef::Focused => Self::Focused,
            WindowRef::Any => Self::Any,
        })
    }

    /// Returns true when the window matches. Unmapped windows match only by id.
    pub fn matches(&self, w: &WindowInfo) -> bool {
        match self {
            Self::Id(id) => w.id == *id,
            _ if !w.is_mapped() => false,
            Self::AppId(app_id) => match w.kind {
                WindowKind::Toplevel => w.app_id == *app_id,
                _ => w.namespace() == Some(app_id.as_str()),
            },
            Self::Layer(namespace) => w.namespace() == Some(namespace.as_str()),
            Self::Lock => w.kind == WindowKind::Lock,
            Self::Title(re) => re.is_match(&w.title),
            Self::Focused => w.focused,
            Self::Any => true,
        }
    }

    /// Returns the first matching window, in mapping order.
    pub fn select<'a>(&self, windows: &'a [WindowInfo]) -> Option<&'a WindowInfo> {
        windows.iter().find(|w| self.matches(w))
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::{OutputId, Rect, SurfaceId, Timestamp, WindowId};

    use super::*;
    use crate::shell::{KeyboardMode, LayerInfo, Placement, ShellLayer};

    fn win(id: u64, app_id: &str, title: &str, focused: bool, mapped: bool) -> WindowInfo {
        WindowInfo {
            id: WindowId(id),
            app_id: app_id.into(),
            title: title.into(),
            pid: None,
            app: None,
            output: OutputId(0),
            geometry: Rect::new(0, 0, 100, 100),
            surface: SurfaceId(id),
            mapped_at: mapped.then_some(Timestamp(id)),
            focused,
            suspended: false,
            placement: Placement::Maximized,
            kind: WindowKind::Toplevel,
            layer: None,
        }
    }

    fn layer(id: u64, namespace: &str) -> WindowInfo {
        WindowInfo {
            kind: WindowKind::Layer,
            layer: Some(LayerInfo {
                namespace: namespace.into(),
                layer: ShellLayer::Top,
                keyboard: KeyboardMode::None,
                exclusive_zone: 32,
                anchor: vec!["top".into()],
            }),
            app_id: String::new(),
            title: String::new(),
            ..win(id, "", "", false, true)
        }
    }

    #[test]
    fn selects_by_each_reference() {
        let windows = vec![
            win(1, "org.a", "Alpha - editor", false, true),
            win(2, "org.b", "Beta", true, true),
            win(3, "org.c", "Gamma", false, false),
        ];
        let pick = |r: WindowRef| {
            WindowMatcher::new(&r)
                .unwrap()
                .select(&windows)
                .map(|w| w.id.0)
        };
        assert_eq!(pick(WindowRef::app_id("org.b")), Some(2));
        assert_eq!(pick(WindowRef::title_regex("^Alpha")), Some(1));
        assert_eq!(pick(WindowRef::Focused), Some(2));
        assert_eq!(pick(WindowRef::Any), Some(1));
        assert_eq!(pick(WindowRef::app_id("org.c")), None);
        assert_eq!(pick(WindowRef::id(WindowId(3))), Some(3));
    }

    #[test]
    fn selects_layers_by_namespace() {
        let windows = vec![win(1, "org.a", "Alpha", false, true), layer(2, "bar")];
        let pick = |r: WindowRef| {
            WindowMatcher::new(&r)
                .unwrap()
                .select(&windows)
                .map(|w| w.id.0)
        };
        assert_eq!(pick(WindowRef::layer("bar")), Some(2));
        assert_eq!(pick(WindowRef::app_id("bar")), Some(2));
        assert_eq!(pick(WindowRef::layer("org.a")), None);
        assert_eq!(pick(WindowRef::Lock), None);
    }

    #[test]
    fn bad_pattern_is_an_error() {
        assert!(WindowMatcher::new(&WindowRef::title_regex("(")).is_err());
    }
}
