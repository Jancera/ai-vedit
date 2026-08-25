use assert_cmd::Command;
use predicates::prelude::*;

fn write_fixture_audio(dir: &std::path::Path) -> std::path::PathBuf {
    let audio_path = dir.join("script.mp3");
    std::fs::write(&audio_path, b"fake mp3 bytes for testing").unwrap();
    audio_path
}

fn mock_successful_transcription(server: &mut mockito::Server) -> mockito::Mock {
    server
        .mock("POST", "/v1/audio/transcriptions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"text":"hi","segments":[{"start":0.0,"end":2.0,"text":"hi"}]}"#)
        .create()
}

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
fn plan_with_missing_audio_file_fails() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", "/nonexistent/does-not-exist.mp3"]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("does-not-exist.mp3"));
}

#[test]
fn plan_without_api_key_fails_with_clear_message() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env_remove("OPENAI_API_KEY");
    cmd.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("OPENAI_API_KEY"));
}

#[test]
fn plan_with_empty_api_key_fails() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "");
    cmd.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("OPENAI_API_KEY"));
}

#[test]
fn plan_defaults_to_16_9_aspect() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Sixteen9"));
}

#[test]
fn plan_accepts_9_16_aspect() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args([
        "plan",
        "--audio",
        audio_path.to_str().unwrap(),
        "--aspect",
        "9:16",
    ]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Nine16"));
}

#[test]
fn plan_defaults_to_assets_dir() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("assets=\"assets\""));
}

#[test]
fn plan_prints_transcription_summary_and_cache_path() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("1 segment"))
        .stdout(predicate::str::contains(".cache"));
}

#[test]
fn plan_reuses_cached_transcript_on_second_run() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let mock = mock_successful_transcription(&mut server).expect(1);

    for _ in 0..2 {
        let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
        cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
        cmd.env("OPENAI_API_KEY", "test-key");
        cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
        cmd.assert().success();
    }

    mock.assert();
}

#[test]
fn plan_continues_when_cache_write_fails() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    // Block the .cache directory from being created by occupying its path with a plain file.
    std::fs::write(dir.path().join(".cache"), b"not a directory").unwrap();

    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("warning"))
        .stdout(predicate::str::contains("1 segment"));
}

#[test]
fn plan_surfaces_transcription_api_error() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = server
        .mock("POST", "/v1/audio/transcriptions")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":{"message":"Invalid API key"}}"#)
        .create();

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Invalid API key"));
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
