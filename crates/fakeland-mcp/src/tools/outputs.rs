//! Output tools: runtime changes, hotplug and replug.

use anyhow::{Context, Result, bail};
use fakeland::OutputChange;
use fakeland_control::Controller;
use fakeland_control::methods::{
    AddOutputParams as CtlAddOutput, NoParams, RemoveOutputParams as CtlRemoveOutput,
    ReplugOutputParams,
};
use fakeland_control::spec::{new_output, parse_output, parse_timing};
use fakeland_control::text::{outputs_text, unplugged_text};
use fakeland_types::OutputId;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use crate::params::{AddOutputParams, RemoveOutputParams, SetOutputParams};
use crate::reply::{ToolResult, done, finish};
use crate::server::FakelandMcp;
use crate::state::{CALL_SLACK, bounded};

impl FakelandMcp {
    async fn output_changes(
        &self,
        c: &Controller,
        p: &SetOutputParams,
    ) -> Result<Vec<OutputChange>> {
        let mut changes = Vec::new();
        let mut refresh = None;
        if let Some(m) = &p.mode {
            let (_, mode) = parse_output(m)?;
            refresh = Some(mode.refresh);
            changes.push(OutputChange::Mode(mode));
        }
        if let Some(s) = p.scale {
            changes.push(OutputChange::Scale { scale: s });
        }
        if let Some(t) = &p.timing {
            let refresh = match refresh {
                Some(r) => r,
                None => {
                    let outputs = bounded(CALL_SLACK, c.session().outputs()).await??;
                    outputs
                        .iter()
                        .find(|o| p.output.is_none_or(|id| o.id == OutputId(id)))
                        .map(|o| o.mode.refresh)
                        .context("no such output")?
                }
            };
            changes.push(OutputChange::Timing(parse_timing(t, refresh)?));
        }
        match p.enabled {
            Some(true) => changes.push(OutputChange::Enable),
            Some(false) => changes.push(OutputChange::Disable),
            None => {}
        }
        if changes.is_empty() {
            bail!("give at least one of mode, scale, timing or enabled");
        }
        Ok(changes)
    }

    /// Returns the outputs and the unplugged outputs as text.
    async fn outputs_reply(&self, c: &Controller) -> Result<String> {
        let outputs = bounded(CALL_SLACK, c.session().outputs()).await??;
        let unplugged = bounded(CALL_SLACK, c.unplugged_outputs(NoParams {})).await??;
        Ok(format!(
            "Outputs:\n{}{}",
            outputs_text(&outputs),
            unplugged_text(&unplugged)
        ))
    }
}

#[tool_router(router = outputs_router, vis = "pub(crate)")]
impl FakelandMcp {
    /// Changes an output.
    #[tool(
        description = "Change an output: mode like \"2560x1440@144\", scale like 1.5, timing (fixed, vrr:48-144, unthrottled, manual), or enabled. Windows follow the new size."
    )]
    async fn set_output(&self, Parameters(p): Parameters<SetOutputParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let output = p.output.map(OutputId);
                for change in self.output_changes(&c, &p).await? {
                    bounded(CALL_SLACK, c.session().set_output(output, change)).await??;
                }
                done(self.outputs_reply(&c).await?)
            }
            .await,
        )
    }

    /// Plugs in an output.
    #[tool(
        description = "Plug in an output. With spec like \"HDMI-A-1:2560x1440@144\" it adds a new output, with optional scale and position [x, y]. Without spec it plugs in an unplugged output again with its name, identity, mode, scale and position: the one named by name, else the latest one. Apps see a new wl_output global."
    )]
    async fn add_output(&self, Parameters(p): Parameters<AddOutputParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let state = match &p.spec {
                    Some(spec) => {
                        let outputs = bounded(CALL_SLACK, c.session().outputs()).await??;
                        let taken: Vec<String> = outputs.into_iter().map(|o| o.name).collect();
                        let mut config = new_output(spec, &taken)?;
                        if let Some(s) = p.scale {
                            config = config.scale(s);
                        }
                        if let Some([x, y]) = p.position {
                            config = config.position(x, y);
                        }
                        bounded(CALL_SLACK, c.add_output(CtlAddOutput { config })).await??
                    }
                    None => {
                        let name = p.name.clone();
                        bounded(CALL_SLACK, c.replug_output(ReplugOutputParams { name })).await??
                    }
                };
                done(format!(
                    "Plugged in {} as output {}.\n{}",
                    state.name,
                    state.id.0,
                    self.outputs_reply(&c).await?
                ))
            }
            .await,
        )
    }

    /// Unplugs an output.
    #[tool(
        description = "Unplug an output: its wl_output global goes away, its layer-shell surfaces close and its windows move to the primary output. The last output stays. add_output without spec plugs it in again."
    )]
    async fn remove_output(&self, Parameters(p): Parameters<RemoveOutputParams>) -> ToolResult {
        finish(
            async {
                let c = self.ctl().await?;
                let output = OutputId(p.output);
                bounded(CALL_SLACK, c.remove_output(CtlRemoveOutput { output })).await??;
                done(format!(
                    "Unplugged output {}.\n{}",
                    p.output,
                    self.outputs_reply(&c).await?
                ))
            }
            .await,
        )
    }
}
