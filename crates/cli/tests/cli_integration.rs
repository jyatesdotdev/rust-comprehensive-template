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
    cmd()
        .args(["greet", "X", "-n", "0"])
        .assert()
        .failure();
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

#[test]
fn serve_defaults() {
    cmd()
        .args(["serve"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Starting server on 127.0.0.1:8080"));
}

#[test]
fn completions_bash() {
    cmd()
        .args(["completions", "bash"])
        .assert()
        .success();
}

#[test]
fn verbose_flag_prints_debug() {
    cmd()
        .args(["--verbose", "greet", "Test"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Cli"));
}
