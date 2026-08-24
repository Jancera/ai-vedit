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
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("4:3"));
}

#[test]
fn plan_accepts_9_16_aspect() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", "script.mp3", "--aspect", "9:16"]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Nine16"));
}

#[test]
fn plan_defaults_to_16_9_aspect() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", "script.mp3"]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Sixteen9"));
}

#[test]
fn plan_defaults_to_assets_dir() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", "script.mp3"]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("assets=\"assets\""));
}

#[test]
fn plan_without_api_key_fails_with_clear_message() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", "script.mp3"]);
    cmd.env_remove("OPENAI_API_KEY");
    cmd.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("OPENAI_API_KEY"));
}

#[test]
fn plan_with_empty_api_key_fails() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", "script.mp3"]);
    cmd.env("OPENAI_API_KEY", "");
    cmd.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("OPENAI_API_KEY"));
}

#[test]
fn render_without_plan_arg_fails() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.arg("render");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--plan"));
}

#[test]
fn render_uses_default_assets_and_out() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["render", "--plan", "plan.json"]);
    cmd.env_remove("OPENAI_API_KEY");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("assets=\"assets\""))
        .stdout(predicate::str::contains("out=\"output.mp4\""));
}

#[test]
fn render_does_not_require_api_key() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["render", "--plan", "plan.json"]);
    cmd.env_remove("OPENAI_API_KEY");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("render: not yet implemented"));
}
