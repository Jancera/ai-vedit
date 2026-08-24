use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_subcommands() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("plan"))
        .stdout(predicate::str::contains("render"));
}

#[test]
fn plan_without_audio_arg_fails() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.arg("plan");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--audio"));
}

#[test]
fn plan_rejects_invalid_aspect() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", "script.mp3", "--aspect", "4:3"]);
    cmd.assert().failure();
}

#[test]
fn plan_defaults_to_16_9_aspect() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", "script.mp3"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Sixteen9"));
}
