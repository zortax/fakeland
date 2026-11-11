//! Trigger sets: a run that sleeps for the set lifetime and holds the triggers.

use fakeland::TriggerFiring;
use fakeland_scenario::{Action, Scenario, Step};

use super::{Controller, OpError};
use crate::methods::{InstallTriggersParams, RemoveTriggersParams, TriggerSet};

impl Controller {
    /// Arms a trigger set.
    pub async fn install_triggers(&self, p: InstallTriggersParams) -> Result<TriggerSet, OpError> {
        if p.triggers.is_empty() {
            return Err(OpError::InvalidParams("the set has no triggers".into()));
        }
        if p.lifetime.0.as_nanos() <= 0 {
            return Err(OpError::InvalidParams(
                "the lifetime must be positive".into(),
            ));
        }
        let scenario = Scenario {
            name: p.name.unwrap_or_else(|| "triggers".into()),
            steps: vec![Step::new(Action::sleep(p.lifetime))],
            triggers: p.triggers,
            timeout: None,
        };
        scenario
            .validate()
            .map_err(|e| OpError::InvalidParams(e.to_string()))?;
        let set = self.start_plain_run(scenario).await?;
        Ok(TriggerSet { set })
    }

    /// Disarms a trigger set and returns its firings.
    pub async fn remove_triggers(
        &self,
        p: RemoveTriggersParams,
    ) -> Result<Vec<TriggerFiring>, OpError> {
        self.session().cancel(p.set).await?;
        Ok(self.wait_run(p.set).await?.triggers)
    }
}
