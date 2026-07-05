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
    // Fall back to the default style if the template is ever invalid —
    // library code paths must not panic (workspace convention).
    if let Ok(style) = ProgressStyle::default_bar()
        .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
    {
        pb.set_style(style.progress_chars("=>-"));
    }
    for i in 0..steps {
        pb.set_message(format!("step {}", i + 1));
        thread::sleep(Duration::from_millis(80));
        pb.inc(1);
    }
    pb.finish_with_message("done");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_colors_contains_all_lines() {
        let out = demo_colors();
        assert!(out.contains("Error: something went wrong"));
        assert!(out.contains("Warning: check your config"));
        assert!(out.contains("Success: all checks passed"));
        assert!(out.contains("Info: processing items..."));
        assert!(out.contains("Debug: verbose details here"));
        assert_eq!(out.lines().count(), 6);
    }

    #[test]
    fn demo_progress_zero_steps_completes() {
        // Zero steps skips the sleep loop entirely — keeps the test fast
        // and deterministic while still exercising bar setup/teardown.
        demo_progress(0);
    }
}
