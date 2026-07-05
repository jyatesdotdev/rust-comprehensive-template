//! Binary entry point for the demo CLI.

use clap::Parser;
use cli::{run, Cli};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.verbose {
        eprintln!("{cli:#?}");
    }
    match run(&cli) {
        Ok(output) => {
            // Some commands (e.g. `completions`) write to stdout themselves and
            // return an empty string — avoid printing a stray blank line for them.
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            // Errors go to stderr, never stdout: stdout is the command's data
            // stream and may be piped or parsed.
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
