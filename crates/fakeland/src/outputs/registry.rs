//! The set of faked outputs in configuration order, and the configs of unplugged outputs that
//! can come back.

use std::collections::BTreeMap;

use fakeland_types::OutputId;
use smithay::output::Output;

use super::FakeOutput;
use crate::config::{OutputConfig, OutputPosition};

/// Every faked output.
#[derive(Debug, Default)]
pub struct Outputs {
    list: BTreeMap<OutputId, FakeOutput>,
    order: Vec<OutputId>,
    unplugged: Vec<OutputConfig>,
}

impl Outputs {
    /// Adds an output at the end of the order.
    pub fn insert(&mut self, output: FakeOutput) {
        self.order.push(output.id);
        self.list.insert(output.id, output);
    }

    /// Removes an output.
    pub fn remove(&mut self, id: OutputId) -> Option<FakeOutput> {
        self.order.retain(|o| *o != id);
        self.list.remove(&id)
    }

    /// Returns an output.
    pub fn get(&self, id: OutputId) -> Option<&FakeOutput> {
        self.list.get(&id)
    }

    /// Returns an output for changes.
    pub fn get_mut(&mut self, id: OutputId) -> Option<&mut FakeOutput> {
        self.list.get_mut(&id)
    }

    /// Returns the outputs in order.
    pub fn iter(&self) -> impl Iterator<Item = &FakeOutput> {
        self.order.iter().filter_map(|id| self.list.get(id))
    }

    /// Returns the count of outputs.
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Returns the primary output: the first enabled output, or the first output.
    pub fn primary(&self) -> Option<OutputId> {
        self.iter()
            .find(|o| o.enabled())
            .or_else(|| self.iter().next())
            .map(|o| o.id)
    }

    /// Resolves an optional output reference. `None` selects the primary output.
    pub fn resolve(&self, id: Option<OutputId>) -> Option<OutputId> {
        match id {
            Some(id) => self.list.contains_key(&id).then_some(id),
            None => self.primary(),
        }
    }

    /// Returns the id of a smithay output.
    pub fn id_of(&self, output: &Output) -> Option<OutputId> {
        self.list
            .values()
            .find(|o| o.output == *output)
            .map(|o| o.id)
    }

    /// Returns the id of an output by connector name.
    pub fn by_name(&self, name: &str) -> Option<OutputId> {
        self.list
            .values()
            .find(|o| o.config.name == name)
            .map(|o| o.id)
    }

    /// Returns the position of a new output: its configured position, or right of the rightmost
    /// output.
    pub fn place(&self, config: &OutputConfig) -> OutputPosition {
        if let Some(p) = config.position {
            return p;
        }
        let x = self
            .iter()
            .map(|o| {
                let r = o.logical_rect();
                r.x + r.w
            })
            .max()
            .unwrap_or(0);
        OutputPosition { x, y: 0 }
    }

    /// Keeps the config of an unplugged output. It replaces an older config of the same name.
    pub fn park(&mut self, config: OutputConfig) {
        self.unplugged.retain(|c| c.name != config.name);
        self.unplugged.push(config);
    }

    /// Returns the config of an unplugged output by name, or the latest one for `None`.
    pub fn parked(&self, name: Option<&str>) -> Option<&OutputConfig> {
        match name {
            Some(name) => self.unplugged.iter().find(|c| c.name == name),
            None => self.unplugged.last(),
        }
    }

    /// Drops the config of an unplugged output.
    pub fn unpark(&mut self, name: &str) {
        self.unplugged.retain(|c| c.name != name);
    }

    /// Returns the configs of the unplugged outputs, the latest last.
    pub fn unplugged(&self) -> &[OutputConfig] {
        &self.unplugged
    }
}
