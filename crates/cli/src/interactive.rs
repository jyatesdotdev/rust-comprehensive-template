//! Interactive CLI features: colored output and progress bars.

use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::thread;
use std::time::Duration;

/// Print colored output examples using owo-colors.
pub fn demo_colors() -> String {
    // owo-colors applies ANSI codes; collect lines for display
    let lines = vec![
        format!("{}", "Error: something went wrong".red().bold()),
        format!("{}", "Warning: check your config".yellow()),
        format!("{}", "Success: all checks passed".green()),
        format!("{}", "Info: processing items...".blue()),
        format!("{}", "Debug: verbose details here".dimmed()),
        format!(
            "Styled: {} {} {}",
            "bold".bold(),
            "italic".italic(),
            "underline".underline()
        ),
    ];
    lines.join("\n")
}

/// Run a progress bar demo using indicatif.
pub fn demo_progress(steps: u64) {
    let pb = ProgressBar::new(steps);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .expect("valid template")
            .progress_chars("=>-"),
    );
    for i in 0..steps {
        pb.set_message(format!("step {}", i + 1));
        thread::sleep(Duration::from_millis(80));
        pb.inc(1);
    }
    pb.finish_with_message("done");
}
