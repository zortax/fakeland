//! Validity checks and their outcome.

use serde::{Deserialize, Serialize};

/// The outcome of one check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// The check passed.
    Pass,
    /// The result is usable, with a caveat.
    Warn,
    /// The result is invalid.
    Fail,
}

impl CheckStatus {
    /// Returns the short label for text reports.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Warn => "warn",
            Self::Fail => "FAIL",
        }
    }
}

/// One validity check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Check {
    /// The check name, such as `wake_lateness`.
    pub name: String,
    /// The outcome.
    pub status: CheckStatus,
    /// What the check found.
    pub message: String,
    /// The measured value.
    #[serde(default)]
    pub value: Option<f64>,
    /// The limit of the value.
    #[serde(default)]
    pub limit: Option<f64>,
}

impl Check {
    /// Builds a check.
    pub fn new(name: &str, status: CheckStatus, message: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status,
            message: message.into(),
            value: None,
            limit: None,
        }
    }

    /// Sets the value and its limit.
    pub fn with_value(mut self, value: f64, limit: f64) -> Self {
        self.value = Some(value);
        self.limit = Some(limit);
        self
    }
}

/// Every check of a run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Validity {
    /// No check failed.
    pub valid: bool,
    /// The checks in order.
    pub checks: Vec<Check>,
}

impl Validity {
    /// Builds the validity from checks.
    pub fn from_checks(checks: Vec<Check>) -> Self {
        Self {
            valid: checks.iter().all(|c| c.status != CheckStatus::Fail),
            checks,
        }
    }

    /// Returns the checks with a status.
    pub fn with_status(&self, status: CheckStatus) -> impl Iterator<Item = &Check> + '_ {
        self.checks.iter().filter(move |c| c.status == status)
    }

    /// Returns a check by name.
    pub fn check(&self, name: &str) -> Option<&Check> {
        self.checks.iter().find(|c| c.name == name)
    }
}
