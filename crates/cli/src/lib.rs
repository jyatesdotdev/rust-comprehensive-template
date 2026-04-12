//! CLI type definitions using clap derive macros.
//!
//! Demonstrates: subcommands, flags, positional/named args, value validation,
//! env var binding, and default values.

pub mod completions;
pub mod config;
pub mod interactive;

use clap::{Parser, Subcommand, Args, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

/// Demo CLI showcasing clap derive-macro patterns.
#[derive(Parser, Debug)]
#[command(name = "demo-cli", version, about = "Rust CLI examples with clap")]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    /// Path to TOML config file
    #[arg(short, long, global = true, default_value = "config.toml")]
    pub config: PathBuf,

    /// Subcommand to execute.
    #[command(subcommand)]
    pub command: Command,
}

/// Supported output formats for CLI responses.
#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable plain text.
    Text,
    /// Machine-readable JSON.
    Json,
}

/// Top-level CLI subcommands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Greet someone (demonstrates positional args and flags)
    Greet(GreetArgs),

    /// Start a server (demonstrates env var binding and value validation)
    Serve(ServeArgs),

    /// Manage configuration (demonstrates nested subcommands)
    Config(ConfigCmd),

    /// Interactive CLI demos (colored output, progress bars)
    Demo(DemoCmd),

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

/// Arguments for the `greet` subcommand.
#[derive(Args, Debug)]
pub struct GreetArgs {
    /// Name to greet
    pub name: String,

    /// Number of times to repeat the greeting
    #[arg(short = 'n', long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=100))]
    pub count: u32,

    /// Use uppercase
    #[arg(short, long)]
    pub uppercase: bool,
}

/// Arguments for the `serve` subcommand.
#[derive(Args, Debug)]
pub struct ServeArgs {
    /// Host to bind to (reads from HOST env var)
    #[arg(long, default_value = "127.0.0.1", env = "HOST")]
    pub host: String,

    /// Port to listen on (reads from PORT env var, must be 1-65535)
    #[arg(long, default_value_t = 8080, env = "PORT", value_parser = clap::value_parser!(u16).range(1..))]
    pub port: u16,

    /// TLS certificate file
    #[arg(long, value_name = "FILE")]
    pub tls_cert: Option<PathBuf>,
}

/// Nested subcommand for `config`.
#[derive(Args, Debug)]
pub struct ConfigCmd {
    /// Config action to perform.
    #[command(subcommand)]
    pub action: ConfigAction,
}

/// Available actions for the `config` subcommand.
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Get a config value
    Get {
        /// Config key to read
        key: String,
    },
    /// Set a config value
    Set {
        /// Config key
        key: String,
        /// Config value
        value: String,
    },
    /// List all config values
    List,
}

/// Nested subcommand for `demo`.
#[derive(Args, Debug)]
pub struct DemoCmd {
    /// Demo action to run.
    #[command(subcommand)]
    pub action: DemoAction,
}

/// Available interactive demo actions.
#[derive(Subcommand, Debug)]
pub enum DemoAction {
    /// Show colored output examples
    Colors,
    /// Run a progress bar animation
    Progress {
        /// Number of steps
        #[arg(short, long, default_value_t = 20)]
        steps: u64,
    },
}

/// Execute the parsed CLI command, returning output as a String.
pub fn run(cli: &Cli) -> String {
    match &cli.command {
        Command::Greet(args) => {
            let mut greeting = format!("Hello, {}!", args.name);
            if args.uppercase {
                greeting = greeting.to_uppercase();
            }
            std::iter::repeat(greeting)
                .take(args.count as usize)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Command::Serve(args) => {
            // Merge config file + env vars + CLI flags via figment
            let overrides = config::CliOverrides {
                host: Some(args.host.clone()),
                port: Some(args.port),
                tls_cert: args.tls_cert.clone(),
            };
            match config::load_config(&cli.config, overrides) {
                Ok(cfg) => format!("Starting server on {}:{} (workers={}, log={})", cfg.host, cfg.port, cfg.workers, cfg.log_level),
                Err(e) => format!("Config error: {e}"),
            }
        }
        Command::Config(cmd) => match &cmd.action {
            ConfigAction::Get { key } => format!("config.get({key})"),
            ConfigAction::Set { key, value } => format!("config.set({key}, {value})"),
            ConfigAction::List => "config.list()".to_string(),
        },
        Command::Demo(cmd) => match &cmd.action {
            DemoAction::Colors => interactive::demo_colors(),
            DemoAction::Progress { steps } => {
                interactive::demo_progress(*steps);
                "Progress complete.".to_string()
            }
        },
        Command::Completions { shell } => {
            completions::print_completions(*shell);
            String::new()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> Cli {
        Cli::parse_from(args)
    }

    #[test]
    fn greet_basic() {
        let cli = parse(&["demo-cli", "greet", "Alice"]);
        match cli.command {
            Command::Greet(ref a) => {
                assert_eq!(a.name, "Alice");
                assert_eq!(a.count, 1);
                assert!(!a.uppercase);
            }
            _ => panic!("expected Greet"),
        }
        assert_eq!(run(&cli), "Hello, Alice!");
    }

    #[test]
    fn greet_with_flags() {
        let cli = parse(&["demo-cli", "greet", "Bob", "-n", "3", "--uppercase"]);
        match &cli.command {
            Command::Greet(a) => {
                assert_eq!(a.count, 3);
                assert!(a.uppercase);
            }
            _ => panic!("expected Greet"),
        }
        assert_eq!(run(&cli), "HELLO, BOB!\nHELLO, BOB!\nHELLO, BOB!");
    }

    #[test]
    fn greet_count_validation_rejects_zero() {
        let result = Cli::try_parse_from(["demo-cli", "greet", "X", "-n", "0"]);
        assert!(result.is_err());
    }

    #[test]
    fn greet_count_validation_rejects_over_100() {
        let result = Cli::try_parse_from(["demo-cli", "greet", "X", "-n", "101"]);
        assert!(result.is_err());
    }

    #[test]
    fn serve_defaults() {
        let cli = parse(&["demo-cli", "serve"]);
        match &cli.command {
            Command::Serve(a) => {
                assert_eq!(a.host, "127.0.0.1");
                assert_eq!(a.port, 8080);
                assert!(a.tls_cert.is_none());
            }
            _ => panic!("expected Serve"),
        }
    }

    #[test]
    fn serve_custom() {
        let cli = parse(&["demo-cli", "serve", "--host", "0.0.0.0", "--port", "3000"]);
        match &cli.command {
            Command::Serve(a) => {
                assert_eq!(a.host, "0.0.0.0");
                assert_eq!(a.port, 3000);
            }
            _ => panic!("expected Serve"),
        }
    }

    #[test]
    fn serve_port_zero_rejected() {
        let result = Cli::try_parse_from(["demo-cli", "serve", "--port", "0"]);
        assert!(result.is_err());
    }

    #[test]
    fn config_get() {
        let cli = parse(&["demo-cli", "config", "get", "key1"]);
        assert_eq!(run(&cli), "config.get(key1)");
    }

    #[test]
    fn config_set() {
        let cli = parse(&["demo-cli", "config", "set", "key1", "val1"]);
        assert_eq!(run(&cli), "config.set(key1, val1)");
    }

    #[test]
    fn config_list() {
        let cli = parse(&["demo-cli", "config", "list"]);
        assert_eq!(run(&cli), "config.list()");
    }

    #[test]
    fn global_verbose_flag() {
        let cli = parse(&["demo-cli", "--verbose", "greet", "X"]);
        assert!(cli.verbose);
    }

    #[test]
    fn global_format_json() {
        let cli = parse(&["demo-cli", "--format", "json", "greet", "X"]);
        assert!(matches!(cli.format, OutputFormat::Json));
    }

    #[test]
    fn completions_parse() {
        let cli = parse(&["demo-cli", "completions", "bash"]);
        assert!(matches!(cli.command, Command::Completions { .. }));
    }
}
