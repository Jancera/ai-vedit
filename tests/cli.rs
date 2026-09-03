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

fn mock_successful_plan(server: &mut mockito::Server) -> mockito::Mock {
    let content = r#"{"beats":[{"start":0.0,"end":2.0,"description":"Opening shot","category":"general","is_new_category":false}]}"#;
    let body = format!(
        r#"{{"choices":[{{"message":{{"content":{}}}}}]}}"#,
        serde_json::to_string(content).unwrap()
    );

    server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
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
fn render_rejects_invalid_aspect() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["render", "--plan", "plan.json", "--aspect", "4:3"]);
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("4:3"));
}

#[test]
fn plan_ignores_unknown_aspect_flag() {
    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.args(["plan", "--audio", "script.mp3", "--aspect", "16:9"]);
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("--aspect"));
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
fn plan_does_not_write_an_aspect_field() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);
    let _plan_mock = mock_successful_plan(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert().success();

    let plan_json = std::fs::read_to_string(dir.path().join("plan.json")).unwrap();
    assert!(
        !plan_json.contains("aspect"),
        "plan.json should no longer carry an aspect field: {plan_json}"
    );
}

#[test]
fn plan_defaults_to_assets_dir() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);
    let _plan_mock = mock_successful_plan(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("assets library: \"assets\""));
}

#[test]
fn plan_caches_transcript_to_disk() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);
    let _plan_mock = mock_successful_plan(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert().success();

    let cache_dir = dir.path().join(".cache");
    let cached_files: Vec<_> = std::fs::read_dir(&cache_dir)
        .expect("expected .cache directory to exist")
        .collect();
    assert_eq!(
        cached_files.len(),
        1,
        "expected exactly one cached transcript file"
    );
}

#[test]
fn plan_reuses_cached_transcript_on_second_run() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let mock = mock_successful_transcription(&mut server).expect(1);
    let plan_mock = mock_successful_plan(&mut server).expect(2);

    for _ in 0..2 {
        let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
        cmd.current_dir(dir.path());
        cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
        cmd.env("OPENAI_API_KEY", "test-key");
        cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
        cmd.assert().success();
    }

    mock.assert();
    plan_mock.assert();
}

#[test]
fn plan_writes_plan_json_and_prints_time_budget() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);
    let _plan_mock = mock_successful_plan(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("general"))
        .stdout(predicate::str::contains("2.0s"));

    assert!(dir.path().join("plan.json").exists());
}

#[test]
fn plan_lists_new_categories_separately() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);

    let content = r#"{"beats":[{"start":0.0,"end":2.0,"description":"New idea","category":"drone-shots","is_new_category":true}]}"#;
    let body = format!(
        r#"{{"choices":[{{"message":{{"content":{}}}}}]}}"#,
        serde_json::to_string(content).unwrap()
    );
    let _plan_mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create();

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("drone-shots"))
        .stdout(predicate::str::contains("created category directories"));

    assert!(dir.path().join("assets").join("drone-shots").is_dir());
}

#[test]
fn plan_creates_missing_assets_and_category_directories() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);
    let _plan_mock = mock_successful_plan(&mut server);

    assert!(!dir.path().join("assets").exists());

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert().success();

    assert!(dir.path().join("assets").is_dir());
    assert!(dir.path().join("assets").join("general").is_dir());
}

#[test]
fn plan_surfaces_planning_api_error() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);
    let _plan_mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":{"message":"Invalid API key"}}"#)
        .create();

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Invalid API key"));
}

#[test]
fn plan_continues_when_cache_write_fails() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    // Block the .cache directory from being created by occupying its path with a plain file.
    std::fs::write(dir.path().join(".cache"), b"not a directory").unwrap();

    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);
    let _plan_mock = mock_successful_plan(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("warning"));

    assert!(dir.path().join("plan.json").exists());
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
fn plan_rejects_non_positive_min_beat_duration() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args([
        "plan",
        "--audio",
        audio_path.to_str().unwrap(),
        "--min-beat-duration",
        "0",
    ]);
    cmd.assert().failure().stderr(predicate::str::contains(
        "--min-beat-duration must be greater than 0",
    ));
}

#[test]
fn plan_merges_beats_shorter_than_minimum_duration() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);

    let content = r#"{"beats":[
        {"start":0.0,"end":1.0,"description":"Intro","category":"intro","is_new_category":false},
        {"start":1.0,"end":2.0,"description":"More intro","category":"intro","is_new_category":false},
        {"start":2.0,"end":6.0,"description":"Main scene","category":"main-scene","is_new_category":true}
    ]}"#;
    let body = format!(
        r#"{{"choices":[{{"message":{{"content":{}}}}}]}}"#,
        serde_json::to_string(content).unwrap()
    );
    let _plan_mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create();

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args([
        "plan",
        "--audio",
        audio_path.to_str().unwrap(),
        "--min-beat-duration",
        "5",
    ]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert().success();

    let plan_json = std::fs::read_to_string(dir.path().join("plan.json")).unwrap();
    let plan: serde_json::Value = serde_json::from_str(&plan_json).unwrap();
    let beats = plan["beats"].as_array().unwrap();

    assert_eq!(
        beats.len(),
        1,
        "expected the three short beats to merge into one"
    );
    assert_eq!(beats[0]["category"], "main-scene");
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
fn render_fails_cleanly_without_api_key_when_plan_file_is_missing() {
    // `render` now actually loads the plan file (M4), so a missing plan.json
    // at the default location fails with a plan-file error — and, unlike
    // `plan`, it never touches OPENAI_API_KEY to get there.
    let dir = tempfile::tempdir().unwrap();

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["render", "--plan", "plan.json"]);
    cmd.env_remove("OPENAI_API_KEY");
    cmd.assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("failed to load plan file"))
        .stderr(predicate::str::contains("OPENAI_API_KEY").not());
}

#[test]
fn plan_rescales_beats_to_match_the_audio_duration() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();

    let _transcription_mock = server
        .mock("POST", "/v1/audio/transcriptions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"text":"hi","segments":[{"start":0.0,"end":2.0,"text":"hi"}],"duration":10.0}"#,
        )
        .create();
    let _plan_mock = mock_successful_plan(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert().success();

    let plan_json = std::fs::read_to_string(dir.path().join("plan.json")).unwrap();
    let plan: serde_json::Value = serde_json::from_str(&plan_json).unwrap();
    let beats = plan["beats"].as_array().unwrap();

    assert_eq!(beats.len(), 1);
    assert_eq!(beats[0]["duration"], 10.0);
    assert_eq!(beats[0]["end"], 10.0);
}

#[test]
fn plan_warns_when_transcript_duration_is_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = write_fixture_audio(dir.path());
    let mut server = mockito::Server::new();
    let _mock = mock_successful_transcription(&mut server);
    let _plan_mock = mock_successful_plan(&mut server);

    let mut cmd = Command::cargo_bin("ai-vedit").unwrap();
    cmd.current_dir(dir.path());
    cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    cmd.env("OPENAI_API_KEY", "test-key");
    cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    cmd.assert().success().stderr(predicate::str::contains(
        "warning: transcript has no known duration",
    ));
}
