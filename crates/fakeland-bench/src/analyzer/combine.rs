//! Analyzer combinators: rename an analyzer, or run two analyzers as one.

use serde::Serialize;
use serde::ser::SerializeMap;

use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;
use crate::metrics::{Metric, Metrics};

/// An analyzer with another section name.
#[derive(Clone, Debug)]
pub struct Named<A> {
    name: String,
    inner: A,
}

impl<A: Analyzer> Analyzer for Named<A> {
    type Report = A::Report;

    fn name(&self) -> String {
        self.name.clone()
    }

    fn analyze(&self, input: &AnalysisInput) -> Self::Report {
        self.inner.analyze(input)
    }

    fn pool(&self, inputs: &[&AnalysisInput]) -> Option<Self::Report> {
        self.inner.pool(inputs)
    }
}

/// Two analyzers that run as one section. The report holds both reports under their names, and
/// the metrics carry the name of their analyzer as a prefix, such as `frame_pacing.fps`.
#[derive(Clone, Debug)]
pub struct Both<A, B> {
    first: A,
    second: B,
}

/// The report of [`Both`].
#[derive(Clone, Debug)]
pub struct BothReport<A, B> {
    /// The name of the first analyzer.
    pub first_name: String,
    /// The report of the first analyzer.
    pub first: A,
    /// The name of the second analyzer.
    pub second_name: String,
    /// The report of the second analyzer.
    pub second: B,
}

impl<A: Serialize, B: Serialize> Serialize for BothReport<A, B> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry(&self.first_name, &self.first)?;
        map.serialize_entry(&self.second_name, &self.second)?;
        map.end()
    }
}

impl<A: Metrics, B: Metrics> Metrics for BothReport<A, B> {
    fn metrics(&self) -> Vec<Metric> {
        let prefixed = |name: &str, ms: Vec<Metric>| -> Vec<Metric> {
            ms.into_iter()
                .map(|mut m| {
                    m.name = format!("{name}.{}", m.name);
                    m
                })
                .collect()
        };
        let mut out = prefixed(&self.first_name, self.first.metrics());
        out.extend(prefixed(&self.second_name, self.second.metrics()));
        out
    }

    fn samples(&self) -> Option<usize> {
        match (self.first.samples(), self.second.samples()) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0).max(b.unwrap_or(0))),
        }
    }
}

impl<A: Analyzer, B: Analyzer> Analyzer for Both<A, B> {
    type Report = BothReport<A::Report, B::Report>;

    fn name(&self) -> String {
        format!("{}+{}", self.first.name(), self.second.name())
    }

    fn analyze(&self, input: &AnalysisInput) -> Self::Report {
        BothReport {
            first_name: self.first.name(),
            first: self.first.analyze(input),
            second_name: self.second.name(),
            second: self.second.analyze(input),
        }
    }

    fn pool(&self, inputs: &[&AnalysisInput]) -> Option<Self::Report> {
        Some(BothReport {
            first_name: self.first.name(),
            first: self.first.pool(inputs)?,
            second_name: self.second.name(),
            second: self.second.pool(inputs)?,
        })
    }
}

/// Combinator methods for every analyzer.
pub trait AnalyzerExt: Analyzer + Sized {
    /// Runs another analyzer in the same section.
    fn and<B: Analyzer>(self, other: B) -> Both<Self, B> {
        Both {
            first: self,
            second: other,
        }
    }

    /// Renames the section, for example to run one analyzer twice with different settings.
    fn named(self, name: impl Into<String>) -> Named<Self> {
        Named {
            name: name.into(),
            inner: self,
        }
    }
}

impl<A: Analyzer> AnalyzerExt for A {}
