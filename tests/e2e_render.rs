use assert_cmd::Command;
use std::path::Path;

fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok()
}

fn ffprobe_available() -> bool {
    std::process::Command::new("ffprobe")
        .arg("-version")
        .output()
        .is_ok()
}

fn generate_fixture_audio(path: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=44100:cl=mono",
            "-t",
            "3",
            "-q:a",
            "9",
            "-acodec",
            "libmp3lame",
        ])
        .arg(path)
        .status()
        .expect("failed to run ffmpeg to generate fixture audio");
    assert!(status.success(), "ffmpeg failed to generate fixture audio");
}

fn generate_fixture_image(path: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=320x240:d=1",
            "-frames:v",
            "1",
        ])
        .arg(path)
        .status()
        .expect("failed to run ffmpeg to generate fixture image");
    assert!(status.success(), "ffmpeg failed to generate fixture image");
}

fn generate_fixture_video(path: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=320x240:d=2",
            "-t",
            "2",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(path)
        .status()
        .expect("failed to run ffmpeg to generate fixture video");
    assert!(status.success(), "ffmpeg failed to generate fixture video");
}

fn mock_successful_transcription(server: &mut mockito::Server) -> mockito::Mock {
    server
        .mock("POST", "/v1/audio/transcriptions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"text":"hello world","segments":[{"start":0.0,"end":3.0,"text":"hello world"}]}"#,
        )
        .create()
}

fn mock_successful_plan(server: &mut mockito::Server) -> mockito::Mock {
    let content = r#"{"beats":[{"start":0.0,"end":1.5,"description":"Opening stills","category":"stills","is_new_category":false},{"start":1.5,"end":3.0,"description":"Closing clip","category":"clips","is_new_category":false}]}"#;
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
#[ignore]
fn full_pipeline_produces_playable_video() {
    if !ffmpeg_available() {
        panic!(
            "ffmpeg not found on PATH — this test requires it; only run with \
             `cargo test -- --ignored` when ffmpeg is installed"
        );
    }
    if !ffprobe_available() {
        panic!("ffprobe not found on PATH — this test requires it (normally bundled with ffmpeg)");
    }

    let dir = tempfile::tempdir().unwrap();

    let audio_path = dir.path().join("script.mp3");
    generate_fixture_audio(&audio_path);

    let stills_dir = dir.path().join("assets").join("stills");
    let clips_dir = dir.path().join("assets").join("clips");
    std::fs::create_dir_all(&stills_dir).unwrap();
    std::fs::create_dir_all(&clips_dir).unwrap();
    generate_fixture_image(&stills_dir.join("photo.jpg"));
    generate_fixture_video(&clips_dir.join("clip.mp4"));

    let mut server = mockito::Server::new();
    let _transcription_mock = mock_successful_transcription(&mut server);
    let _plan_mock = mock_successful_plan(&mut server);

    let mut plan_cmd = Command::cargo_bin("ai-vedit").unwrap();
    plan_cmd.current_dir(dir.path());
    plan_cmd.args(["plan", "--audio", audio_path.to_str().unwrap()]);
    plan_cmd.env("OPENAI_API_KEY", "test-key");
    plan_cmd.env("AI_VEDIT_OPENAI_BASE_URL", server.url());
    plan_cmd.assert().success();

    let plan_path = dir.path().join("plan.json");
    assert!(plan_path.exists(), "expected plan.json to be written");

    let output_path = dir.path().join("final.mp4");
    let mut render_cmd = Command::cargo_bin("ai-vedit").unwrap();
    render_cmd.current_dir(dir.path());
    render_cmd.args([
        "render",
        "--plan",
        "plan.json",
        "--out",
        output_path.to_str().unwrap(),
    ]);
    render_cmd.assert().success();

    let metadata = std::fs::metadata(&output_path).expect("expected output video to exist");
    assert!(metadata.len() > 0, "expected non-empty output video");

    let probe_output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
        ])
        .arg(&output_path)
        .output()
        .expect("failed to run ffprobe on output video");
    let stream_types = String::from_utf8_lossy(&probe_output.stdout);

    assert!(
        stream_types.contains("video"),
        "expected output to have a video stream, ffprobe reported: {stream_types}"
    );
    assert!(
        stream_types.contains("audio"),
        "expected output to have an audio stream, ffprobe reported: {stream_types}"
    );
}
