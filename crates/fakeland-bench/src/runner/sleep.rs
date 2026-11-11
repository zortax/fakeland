//! A pause on the compositor clock, as a one-step scenario.

use fakeland::Session;
use fakeland_scenario::Scenario;
use fakeland_types::Dur;

use crate::error::BenchError;

/// Waits for a duration on the compositor thread. The future resolves when a one-step sleep
/// scenario ends.
pub async fn sleep(session: &Session, duration: Dur) -> Result<(), BenchError> {
    if duration.0.as_nanos() <= 0 {
        return Ok(());
    }
    let scenario = Scenario::builder("bench.sleep").sleep(duration).build()?;
    session.run(scenario).await?.await?;
    Ok(())
}
