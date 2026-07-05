//! Binary entry point for the demo CLI.

use clap::Parser;
use cli::{run, Cli};

fn main() {
    let cli = Cli::parse();
    if cli.verbose {
        eprintln!("{cli:#?}");
    }
    let output = run(&cli);
    // Some commands (e.g. `completions`) write to stdout themselves and
    // return an empty string — avoid printing a stray blank line for them.
    if !output.is_empty() {
        println!("{output}");
    }
}
