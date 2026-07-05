//! Integration tests using assert_cmd — runs the compiled binary.

use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("demo-cli").expect("binary exists")
}

#[test]
fn greet_outputs_hello() {
    cmd()
        .args(["greet", "World"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello, World!"));
}

#[test]
fn greet_uppercase() {
    cmd()
        .args(["greet", "World", "--uppercase"])
        .assert()
        .success()
        .stdout(predicate::str::contains("HELLO, WORLD!"));
}

#[test]
fn greet_repeat() {
    cmd()
        .args(["greet", "A", "-n", "3"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Hello, A!\nHello, A!\nHello, A!"));
}

#[test]
fn missing_subcommand_fails() {
    cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn invalid_count_fails() {
    cmd().args(["greet", "X", "-n", "0"]).assert().failure();
}

#[test]
fn help_flag() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Rust CLI examples with clap"));
}

#[test]
fn version_flag() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("demo-cli"));
}

#[test]
fn config_get_subcommand() {
    cmd()
        .args(["config", "get", "mykey"])
        .assert()
        .success()
        .stdout(predicate::str::contains("config.get(mykey)"));
}

/// Strip env vars that `serve` reads (clap `env = ...` bindings and the
/// figment `APP_` layer) so tests stay hermetic regardless of the caller's
/// shell environment.
fn serve_cmd() -> Command {
    let mut c = cmd();
    for var in [
        "HOST",
        "PORT",
        "APP_HOST",
        "APP_PORT",
        "APP_WORKERS",
        "APP_LOG_LEVEL",
        "APP_TLS_CERT",
    ] {
        c.env_remove(var);
    }
    c
}

#[test]
fn serve_defaults() {
    serve_cmd()
        .args(["serve"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Starting server on 127.0.0.1:8080",
        ));
}

#[test]
fn serve_reads_config_file() {
    // workers/log_level have no CLI flag, so they demonstrate the figment
    // file layer end-to-end. Config lives in a temp dir to stay hermetic.
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "workers = 16\nlog_level = \"debug\"\n").expect("write config");

    serve_cmd()
        .args(["--config", path.to_str().expect("utf-8 path"), "serve"])
        .assert()
        .success()
        .stdout(predicate::str::contains("workers=16, log=debug"));
}

#[test]
fn serve_config_file_sets_host_and_port() {
    // host/port flags are Option<T>: unset flags must not shadow the file layer.
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "host = \"0.0.0.0\"\nport = 3000\n").expect("write config");

    serve_cmd()
        .args(["--config", path.to_str().expect("utf-8 path"), "serve"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Starting server on 0.0.0.0:3000"));
}

#[test]
fn serve_cli_flag_overrides_config_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "port = 3000\n").expect("write config");

    serve_cmd()
        .args([
            "--config",
            path.to_str().expect("utf-8 path"),
            "serve",
            "--port",
            "9090",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(":9090"));
}

#[test]
fn serve_env_overrides_config_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "workers = 16\n").expect("write config");

    serve_cmd()
        .args(["--config", path.to_str().expect("utf-8 path"), "serve"])
        .env("APP_WORKERS", "2")
        .assert()
        .success()
        .stdout(predicate::str::contains("workers=2"));
}

#[test]
fn serve_config_error_hits_stderr_with_nonzero_exit() {
    // stdout is the data stream: errors must land on stderr with a failure
    // exit code, never as normal-looking output with exit 0.
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "port = ").expect("write malformed config");

    serve_cmd()
        .args(["--config", path.to_str().expect("utf-8 path"), "serve"])
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("config error"));
}

#[test]
fn completions_bash() {
    cmd().args(["completions", "bash"]).assert().success();
}

#[test]
fn verbose_flag_prints_debug() {
    cmd()
        .args(["--verbose", "greet", "Test"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Cli"));
}
