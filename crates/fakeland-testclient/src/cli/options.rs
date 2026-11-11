//! Validation of the command line and its conversion to scene options and the load plan.

use fakeland_types::Hz;

use super::Args;
use crate::load::LoadPlan;
use crate::pace::PresentMode;
use crate::scene::{SceneKind, SceneOptions};

/// A command line whose options contradict each other.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OptionsError {
    /// `--mode fixed-rate` without `--rate`.
    #[error("--mode fixed-rate needs --rate")]
    MissingRate,
    /// `--rate` with another mode.
    #[error("--rate works only with --mode fixed-rate")]
    RateWithoutFixedRate,
    /// The idle scene with another scene.
    #[error("--scene idle cannot be combined with other scenes")]
    IdleCombined,
    /// `--jank` without `--jank-every`, or the reverse.
    #[error("--jank and --jank-every work only together")]
    HalfJank,
}

impl Args {
    /// Checks that the options fit together.
    pub fn validate(&self) -> Result<(), OptionsError> {
        match (self.mode, self.rate) {
            (PresentMode::FixedRate, None) => return Err(OptionsError::MissingRate),
            (PresentMode::FixedRate, Some(_)) | (_, None) => {}
            (_, Some(_)) => return Err(OptionsError::RateWithoutFixedRate),
        }
        if self.scene.contains(&SceneKind::Idle) && self.scene.iter().any(|s| *s != SceneKind::Idle)
        {
            return Err(OptionsError::IdleCombined);
        }
        if (self.jank_every > 0) != (self.jank.0 > 0) {
            return Err(OptionsError::HalfJank);
        }
        Ok(())
    }

    /// Returns the fixed-rate commit rate.
    pub fn rate_hz(&self) -> Option<Hz> {
        self.rate.map(Hz)
    }

    /// Returns the scene options.
    pub fn scene_options(&self) -> SceneOptions {
        let mut kinds = self.scene.clone();
        kinds.dedup();
        SceneOptions {
            kinds,
            background: self.background,
            hover_rect: self.hover_rect,
            hover_color: self.hover_color,
            hover_active_color: self.hover_active_color,
            click_rect: self.click_rect,
            click_color: self.click_color,
            click_active_color: self.click_active_color,
            scroll_unit: self.scroll_unit,
            wheel_step: self.wheel_step,
            scroll_factor: self.scroll_factor,
            stripe_height: self.stripe_height,
            speed: self.speed,
            bar_width: self.bar_width,
            bar_color: self.bar_color,
            pointer_marker: self.pointer_marker,
            show_frame_id: self.show_frame_id,
        }
    }

    /// Returns the injected load of each frame.
    pub fn load_plan(&self) -> LoadPlan {
        LoadPlan {
            render_delay: self.render_delay,
            jank_every: self.jank_every,
            jank: self.jank,
            cpu_burn: self.cpu_burn,
            kind: self.delay_kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use fakeland_types::{Nanos, Rect, Size};

    use super::*;
    use crate::scene::ScrollUnit;

    fn parse(args: &[&str]) -> Args {
        let mut v = vec!["fakeland-testclient"];
        v.extend_from_slice(args);
        Args::try_parse_from(v).unwrap()
    }

    #[test]
    fn defaults() {
        let a = parse(&[]);
        a.validate().unwrap();
        assert_eq!(a.mode, PresentMode::Callback);
        assert_eq!(a.scene, vec![SceneKind::Hover]);
        assert_eq!(a.size, Size::new(640, 480));
        assert_eq!(a.app_id, "fakeland-testclient");
        assert_eq!(a.buffers, 3);
        assert_eq!(a.render_delay, Nanos::ZERO);
        assert!(a.log_json.is_none());
    }

    #[test]
    fn full_command_line() {
        let a = parse(&[
            "--mode",
            "fixed-rate",
            "--rate",
            "59.94",
            "--scene",
            "scroll,animate",
            "--scene",
            "hover",
            "--render-delay",
            "7",
            "--jank-every",
            "10",
            "--jank",
            "25ms",
            "--hover-rect",
            "10,20,30,40",
            "--scroll-unit",
            "value120",
            "--exit-after",
            "2.5",
            "--log-json",
            "-",
            "--fifo",
            "--target-delay",
            "500us",
        ]);
        a.validate().unwrap();
        assert_eq!(a.rate_hz(), Some(Hz(59.94)));
        assert_eq!(
            a.scene,
            vec![SceneKind::Scroll, SceneKind::Animate, SceneKind::Hover]
        );
        let plan = a.load_plan();
        assert_eq!(plan.min_render_time(10), Nanos::from_millis(32));
        let s = a.scene_options();
        assert_eq!(s.hover_rect, Rect::new(10, 20, 30, 40));
        assert_eq!(s.scroll_unit, ScrollUnit::Value120);
        assert_eq!(a.exit_after, Some(Nanos::from_millis(2500)));
        assert_eq!(a.target_delay, Some(Nanos::from_micros(500)));
        assert!(a.fifo);
    }

    #[test]
    fn contradictions() {
        assert_eq!(
            parse(&["--mode", "fixed-rate"]).validate(),
            Err(OptionsError::MissingRate)
        );
        assert_eq!(
            parse(&["--rate", "60"]).validate(),
            Err(OptionsError::RateWithoutFixedRate)
        );
        assert_eq!(
            parse(&["--scene", "idle,hover"]).validate(),
            Err(OptionsError::IdleCombined)
        );
        assert_eq!(
            parse(&["--jank", "5"]).validate(),
            Err(OptionsError::HalfJank)
        );
        parse(&["--scene", "idle"]).validate().unwrap();
    }

    #[test]
    fn bad_values_fail_to_parse() {
        let bad = |args: &[&str]| {
            let mut v = vec!["fakeland-testclient"];
            v.extend_from_slice(args);
            Args::try_parse_from(v).is_err()
        };
        assert!(bad(&["--size", "0x0"]));
        assert!(bad(&["--buffers", "0"]));
        assert!(bad(&["--mode", "sometimes"]));
        assert!(bad(&["--scene", "dance"]));
        assert!(bad(&["--background", "red"]));
        assert!(bad(&["--rate", "-1"]));
        assert!(bad(&["--exit-after-frames", "0"]));
    }
}
