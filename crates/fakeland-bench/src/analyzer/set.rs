//! A list of analyzers that run on the same iterations.

use std::sync::Arc;

use crate::analyzer::{Analyzer, DynAnalyzer};
use crate::input::AnalysisInput;
use crate::report::Section;

/// Several analyzers that run on the same iterations. Each analyzer gives one section; the
/// analyzers run on parallel threads.
#[derive(Clone, Default)]
pub struct AnalyzerSet {
    analyzers: Vec<Arc<dyn DynAnalyzer>>,
}

impl std::fmt::Debug for AnalyzerSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.analyzers.iter().map(|a| a.section_name()))
            .finish()
    }
}

impl AnalyzerSet {
    /// Builds an empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an analyzer.
    pub fn with<A: Analyzer>(mut self, analyzer: A) -> Self {
        self.push(analyzer);
        self
    }

    /// Adds an analyzer.
    pub fn push<A: Analyzer>(&mut self, analyzer: A) {
        self.analyzers.push(Arc::new(analyzer));
    }

    /// Adds a type-erased analyzer.
    pub fn push_dyn(&mut self, analyzer: Arc<dyn DynAnalyzer>) {
        self.analyzers.push(analyzer);
    }

    /// Returns the analyzers in order.
    pub fn into_analyzers(self) -> impl Iterator<Item = Arc<dyn DynAnalyzer>> {
        self.analyzers.into_iter()
    }

    /// Returns the count of analyzers.
    pub fn len(&self) -> usize {
        self.analyzers.len()
    }

    /// Returns true without analyzers.
    pub fn is_empty(&self) -> bool {
        self.analyzers.is_empty()
    }

    /// Returns the section names in order.
    pub fn names(&self) -> Vec<String> {
        self.analyzers.iter().map(|a| a.section_name()).collect()
    }

    /// Runs every analyzer on the inputs and returns the sections in analyzer order.
    pub fn sections(&self, inputs: &[&AnalysisInput], seed: u64) -> Vec<Section> {
        std::thread::scope(|scope| {
            let handles: Vec<_> = self
                .analyzers
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let a = a.clone();
                    scope.spawn(move || a.section(inputs, seed.wrapping_add(i as u64 * 7919)))
                })
                .collect();
            handles
                .into_iter()
                .zip(&self.analyzers)
                .map(|(h, a)| {
                    h.join()
                        .unwrap_or_else(|_| Section::failed(a.section_name()))
                })
                .collect()
        })
    }
}
