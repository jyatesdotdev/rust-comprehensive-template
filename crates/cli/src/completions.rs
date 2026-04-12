//! Shell completion generation via clap_complete.

use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

use crate::Cli;

/// Write shell completions for the given shell to stdout.
pub fn print_completions(shell: Shell) {
    generate(shell, &mut Cli::command(), "demo-cli", &mut io::stdout());
}
