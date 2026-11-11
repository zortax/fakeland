//! Output changes of a bench and the restore of the outputs afterwards.

use fakeland::{OutputChange, OutputState, Session};
use fakeland_timing::TimingMode;
use fakeland_types::{Nanos, OutputId};

use crate::error::BenchError;

/// Applies output changes. `None` selects `fallback`, the output of the target. A mode change
/// with a zero width or height keeps the current size of the output.
pub async fn apply(
    session: &Session,
    changes: &[(Option<OutputId>, OutputChange)],
    fallback: Option<OutputId>,
) -> Result<(), BenchError> {
    for (output, change) in changes {
        let id = output.or(fallback);
        let change = match change {
            OutputChange::Mode(m) if m.width <= 0 || m.height <= 0 => {
                let states = session.outputs().await?;
                let current = match id {
                    Some(id) => states.iter().find(|s| s.id == id),
                    None => states.first(),
                };
                let mut mode = *m;
                if let Some(c) = current {
                    mode.width = c.mode.width;
                    mode.height = c.mode.height;
                }
                OutputChange::Mode(mode)
            }
            other => other.clone(),
        };
        session.set_output(id, change).await?;
    }
    Ok(())
}

/// Returns the changes that bring outputs back to an earlier state.
pub fn restore_changes(
    before: &[OutputState],
    now: &[OutputState],
) -> Vec<(Option<OutputId>, OutputChange)> {
    let mut out = Vec::new();
    for b in before {
        let Some(n) = now.iter().find(|n| n.id == b.id) else {
            continue;
        };
        let id = Some(b.id);
        if n.timing != b.timing {
            out.push((id, OutputChange::Timing(b.timing)));
        }
        if n.mode != b.mode {
            out.push((id, OutputChange::Mode(b.mode)));
        }
        if n.scale != b.scale {
            out.push((id, OutputChange::Scale { scale: b.scale }));
        }
        if n.transform != b.transform {
            out.push((
                id,
                OutputChange::Transform {
                    transform: b.transform,
                },
            ));
        }
        if n.enabled != b.enabled {
            out.push((
                id,
                if b.enabled {
                    OutputChange::Enable
                } else {
                    OutputChange::Disable
                },
            ));
        }
    }
    out
}

/// Returns the refresh period of an output: the mode refresh on a fixed grid, else the nominal
/// period of the timing mode.
pub fn period_of(state: &OutputState) -> Option<Nanos> {
    match state.timing.mode {
        TimingMode::Fixed { .. } => Some(state.mode.refresh.period()),
        mode => mode.nominal_period(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland::{ModeConfig, OutputPosition};
    use fakeland_scenario::Transform;
    use fakeland_timing::TimingConfig;
    use fakeland_types::{Hz, Size};

    fn state(id: u32, hz: f64, scale: f64) -> OutputState {
        OutputState {
            id: OutputId(id),
            name: format!("DP-{id}"),
            make: String::new(),
            model: String::new(),
            serial: String::new(),
            mode: ModeConfig::new(1920, 1080, Hz(hz)),
            modes: Vec::new(),
            scale,
            transform: Transform::Normal,
            position: OutputPosition { x: 0, y: 0 },
            logical_size: Size::new(1920, 1080),
            timing: TimingConfig::fixed(Hz(hz)),
            enabled: true,
            physical_mm: (600, 340),
        }
    }

    #[test]
    fn restore_undoes_mode_scale_and_timing() {
        let before = vec![state(1, 60.0, 1.0), state(2, 60.0, 1.0)];
        let mut now = before.clone();
        now[0].mode.refresh = Hz(144.0);
        now[0].timing = TimingConfig::unthrottled();
        now[1].scale = 1.5;
        now[1].enabled = false;
        let changes = restore_changes(&before, &now);
        assert_eq!(
            changes,
            vec![
                (
                    Some(OutputId(1)),
                    OutputChange::Timing(TimingConfig::fixed(Hz(60.0)))
                ),
                (
                    Some(OutputId(1)),
                    OutputChange::Mode(ModeConfig::new(1920, 1080, Hz(60.0)))
                ),
                (Some(OutputId(2)), OutputChange::Scale { scale: 1.0 }),
                (Some(OutputId(2)), OutputChange::Enable),
            ]
        );
        assert!(restore_changes(&before, &before).is_empty());
    }

    #[test]
    fn periods_follow_the_timing_mode() {
        let mut s = state(1, 120.0, 1.0);
        assert_eq!(period_of(&s), Some(Hz(120.0).period()));
        s.timing = TimingConfig::unthrottled();
        assert_eq!(period_of(&s), None);
        s.timing = TimingConfig::vrr(Hz(48.0), Hz(144.0));
        assert_eq!(period_of(&s), Some(Hz(144.0).period()));
    }
}
