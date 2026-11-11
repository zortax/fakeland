//! The bench runner: a [`Bench`] builder over a session, the drivers that run one iteration
//! (scenarios, reactors and custom async code), and the [`BenchRun`] with every iteration and the
//! report.

mod assemble;
mod bench;
mod bench_run;
mod capture;
mod context;
mod driver;
mod execute;
mod outputs;
mod phase;
mod reactor_driver;
mod record;
mod scenario_driver;
mod sleep;

pub use bench::Bench;
pub use bench_run::BenchRun;
pub use context::{IterationContext, Variant};
pub use driver::{DriveFuture, Driver, DriverOutput, FnDriver, driver_fn};
pub use phase::anchor_first_step;
pub use reactor_driver::ReactorDriver;
pub use record::IterationRecord;
pub use scenario_driver::ScenarioDriver;
pub use sleep::sleep;
