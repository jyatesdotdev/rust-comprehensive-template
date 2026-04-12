//! Binary entry point for the demo CLI.

use clap::Parser;
use cli::{Cli, run};

fn main() {
    let cli = Cli::parse();
    if cli.verbose {
        eprintln!("{cli:#?}");
    }
    println!("{}", run(&cli));
}
