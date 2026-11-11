//! Errors of the bench runner.

use fakeland::SessionError;
use fakeland_scenario::ScenarioError;
use thiserror::Error;

/// An error that stops a bench run or one of its iterations.
#[derive(Debug, Error)]
pub enum BenchError {
    /// A session call failed.
    #[error(transparent)]
    Session(#[from] SessionError),
    /// A scenario did not validate.
    #[error(transparent)]
    Scenario(#[from] ScenarioError),
    /// The bench has no driver.
    #[error("the bench has no driver: set a scenario, a reactor or a custom driver")]
    NoDriver,
    /// The bench asks for zero measured iterations.
    #[error("the bench needs at least one measured iteration")]
    NoIterations,
    /// A driver failed.
    #[error("driver: {0}")]
    Driver(String),
}
