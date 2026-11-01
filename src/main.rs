use std::process::ExitCode;

use rustcraft::{HELP, LaunchCommand, parse_arguments};

fn main() -> ExitCode {
    match parse_arguments(std::env::args().skip(1)) {
        Ok(LaunchCommand::Help) => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(LaunchCommand::Run(options)) => match rustcraft::run(options) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("RustCraft could not run: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("{error}\nRun rustcraft --help for usage.");
            ExitCode::FAILURE
        }
    }
}
