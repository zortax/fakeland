//! What to launch, how a child ended, and launch errors.

use std::path::PathBuf;

use fakeland_types::OutputId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A command to launch inside the session.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct LaunchSpec {
    /// The program, a path or a name on `PATH`.
    pub program: String,
    /// The arguments.
    pub args: Vec<String>,
    /// More environment variables. They override the session environment.
    pub env: Vec<(String, String)>,
    /// The working directory. `None` keeps the current directory.
    pub cwd: Option<PathBuf>,
    /// The output for the windows of the process. `None` selects the primary output.
    pub output: Option<OutputId>,
}

impl LaunchSpec {
    /// Builds a spec for a program.
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            ..Self::default()
        }
    }

    /// Builds a spec from a command line: the program, then its arguments.
    pub fn from_argv<I, S>(argv: I) -> Option<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut it = argv.into_iter().map(Into::into);
        let program = it.next()?;
        Some(Self {
            program,
            args: it.collect(),
            ..Self::default()
        })
    }

    /// Adds an argument.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Adds arguments.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Adds an environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Sets the working directory.
    pub fn cwd(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    /// Sets the output for the windows of the process.
    pub fn output(mut self, output: OutputId) -> Self {
        self.output = Some(output);
        self
    }

    /// Returns the command line as one string.
    pub fn command_line(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// How a child process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExitInfo {
    /// The exit code of a normal exit.
    pub code: Option<i32>,
    /// The signal that killed the process.
    pub signal: Option<i32>,
}

impl ExitInfo {
    /// Returns true for a zero exit code.
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// A launch that failed.
#[derive(Debug, Error)]
pub enum LaunchError {
    /// The program is empty.
    #[error("empty program")]
    EmptyProgram,
    /// The output does not exist.
    #[error("no output {0}")]
    NoOutput(OutputId),
    /// The process does not start.
    #[error("cannot start {program}: {source}")]
    Spawn {
        /// The program.
        program: String,
        /// The spawn error.
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_from_argv() {
        let spec = LaunchSpec::from_argv(["vkcube", "--wsi", "wayland"]).unwrap();
        assert_eq!(spec.program, "vkcube");
        assert_eq!(spec.args, ["--wsi", "wayland"]);
        assert_eq!(spec.command_line(), "vkcube --wsi wayland");
        assert!(LaunchSpec::from_argv(Vec::<String>::new()).is_none());
    }

    #[test]
    fn round_trips_through_json() {
        let spec = LaunchSpec::new("kitty")
            .arg("-e")
            .env("A", "1")
            .output(OutputId(2));
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(serde_json::from_str::<LaunchSpec>(&json).unwrap(), spec);
    }
}
