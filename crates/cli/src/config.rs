//! Configuration merging: TOML file + env vars + CLI flags via figment.
//!
//! Priority (highest wins): CLI flags > env vars > config file > defaults.

use figment::{Figment, providers::{Format, Toml, Env, Serialized}};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Application configuration, populated from layered sources.
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct AppConfig {
    /// Server bind address.
    pub host: String,
    /// Server listen port.
    pub port: u16,
    /// Optional path to a TLS certificate file.
    pub tls_cert: Option<PathBuf>,
    /// Logging verbosity level (e.g. `"info"`, `"debug"`).
    pub log_level: String,
    /// Number of worker threads.
    pub workers: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 8080,
            tls_cert: None,
            log_level: "info".into(),
            workers: 4,
        }
    }
}

/// CLI overrides — only fields explicitly set by the user should be merged.
/// Uses `Option` so unset fields don't clobber lower-priority layers.
#[derive(Debug, Serialize)]
pub struct CliOverrides {
    /// Override for the server host.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Override for the server port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Override for the TLS certificate path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_cert: Option<PathBuf>,
}

/// Load config by merging: defaults → TOML file → env vars (APP_ prefix) → CLI overrides.
pub fn load_config(config_path: &Path, cli: CliOverrides) -> Result<AppConfig, figment::Error> {
    Figment::new()
        .merge(Serialized::defaults(AppConfig::default()))
        .merge(Toml::file(config_path))
        .merge(Env::prefixed("APP_"))
        .merge(Serialized::defaults(cli))
        .extract()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn defaults_when_no_file_or_overrides() {
        let cfg = load_config(
            Path::new("nonexistent.toml"),
            CliOverrides { host: None, port: None, tls_cert: None },
        )
        .unwrap();
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn toml_overrides_defaults() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "port = 3000\nlog_level = \"debug\"").unwrap();

        let cfg = load_config(
            tmp.path(),
            CliOverrides { host: None, port: None, tls_cert: None },
        )
        .unwrap();
        assert_eq!(cfg.port, 3000);
        assert_eq!(cfg.log_level, "debug");
        assert_eq!(cfg.host, "127.0.0.1"); // default preserved
    }

    #[test]
    fn cli_overrides_toml() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "port = 3000").unwrap();

        let cfg = load_config(
            tmp.path(),
            CliOverrides { host: None, port: Some(9090), tls_cert: None },
        )
        .unwrap();
        assert_eq!(cfg.port, 9090); // CLI wins
    }
}
