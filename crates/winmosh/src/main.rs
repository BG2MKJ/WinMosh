#![forbid(unsafe_code)]

mod cli;
mod doctor;
mod error;
mod session;
mod uninstall;
mod update;

use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::run(std::env::args_os()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
