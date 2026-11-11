//! The bench selection flags shared by `fakeland bench` and `fakeland ctl bench`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;
use fakeland_bench::PresetParams;
use fakeland_control::methods::BenchParams;
use fakeland_control::spec::parse_window_ref;
use fakeland_scenario::Scenario;

/// What to bench and its parameters.
#[derive(Debug, Clone, Args)]
pub struct BenchSelect {
    /// A preset name (see `fakeland presets`), or a scenario JSON file to repeat.
    pub what: String,
    /// A preset parameter as KEY=VALUE; a value that parses as JSON is JSON. Repeat for more.
    #[arg(long = "param", value_name = "KEY=VALUE")]
    pub params: Vec<String>,
    /// Preset parameters as one JSON object. --param values apply on top.
    #[arg(long, value_name = "JSON")]
    pub params_json: Option<String>,
    /// The measured iterations.
    #[arg(long)]
    pub iterations: Option<u32>,
    /// The warmup iterations.
    #[arg(long)]
    pub warmup: Option<u32>,
    /// The window under test: focused, any, ID, app:APP_ID or title:REGEX.
    #[arg(long, value_name = "WINDOW")]
    pub target: Option<String>,
}

impl BenchSelect {
    /// Returns true when `what` names a scenario file.
    pub fn is_scenario(&self) -> bool {
        self.what.ends_with(".json") || Path::new(&self.what).is_file()
    }

    /// Returns the preset parameters.
    pub fn preset_params(&self) -> Result<PresetParams> {
        let mut params = match &self.params_json {
            Some(j) => PresetParams::from_json(j).context("--params-json is no JSON object")?,
            None => PresetParams::new(),
        };
        let pairs = PresetParams::from_pairs(self.params.iter().map(String::as_str))
            .map_err(anyhow::Error::msg)?;
        params.0.extend(pairs.0);
        Ok(params)
    }

    /// Builds the bench parameters without output files.
    pub fn bench_params(&self) -> Result<BenchParams> {
        let mut p = BenchParams {
            iterations: self.iterations,
            warmup: self.warmup,
            target: self.target.as_deref().map(parse_window_ref).transpose()?,
            ..BenchParams::default()
        };
        if self.is_scenario() {
            if !self.params.is_empty() || self.params_json.is_some() {
                bail!("a scenario bench takes --iterations, --warmup and --target, no --param");
            }
            let path = PathBuf::from(&self.what);
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("cannot read {}", path.display()))?;
            p.scenario = Some(
                Scenario::from_json(&text)
                    .with_context(|| format!("{} is no valid scenario", path.display()))?,
            );
        } else {
            p.preset = Some(self.what.clone());
            p.params = self.preset_params()?.to_value();
        }
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select(what: &str, params: &[&str]) -> BenchSelect {
        BenchSelect {
            what: what.into(),
            params: params.iter().map(|s| (*s).to_owned()).collect(),
            params_json: Some(r#"{"iterations": 3, "text": "a"}"#.into()),
            iterations: None,
            warmup: Some(1),
            target: Some("app:x".into()),
        }
    }

    #[test]
    fn merges_preset_params() {
        let p = select("hover-latency", &["iterations=5", "label=hi"])
            .bench_params()
            .unwrap();
        assert_eq!(p.preset.as_deref(), Some("hover-latency"));
        assert_eq!(p.params["iterations"], 5);
        assert_eq!(p.params["text"], "a");
        assert_eq!(p.params["label"], "hi");
        assert_eq!(p.warmup, Some(1));
        assert!(p.target.is_some());
    }

    #[test]
    fn rejects_params_for_scenarios() {
        assert!(select("x.json", &["a=1"]).bench_params().is_err());
    }
}
