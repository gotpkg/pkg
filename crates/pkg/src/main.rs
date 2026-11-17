//! The `pkg` executable: a shell around [`pkg::app::run`].

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(pkg::app::run(&std::env::args_os().collect::<Vec<_>>()))
}
