//! Connects to the socket and runs one ctl command.

use anyhow::{Result, bail};
use fakeland_control::ControlClient;

use super::args::CtlArgs;
use super::{a11y, bench, input, media, session, trace};

/// Runs `fakeland ctl` and returns the process exit code.
pub fn run(args: CtlArgs) -> Result<i32> {
    let client = match &args.socket {
        Some(path) => ControlClient::connect(path)?,
        None => ControlClient::connect_default()?,
    };
    for group in [
        session::run,
        input::run,
        trace::run,
        bench::run,
        a11y::run,
        media::run,
    ] {
        if let Some(code) = group(&client, args.json, &args.command)? {
            return Ok(code);
        }
    }
    bail!("unhandled ctl command")
}
