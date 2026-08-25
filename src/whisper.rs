use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transcript {
    pub text: String,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug)]
pub enum WhisperError {
    Io(std::io::Error),
    Http(String),
    Api { status: u16, message: String },
    Json(serde_json::Error),
}

impl fmt::Display for WhisperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhisperError::Io(e) => write!(f, "failed to read audio file: {e}"),
            WhisperError::Http(e) => write!(f, "request to Whisper API failed: {e}"),
            WhisperError::Api { status, message } => {
                write!(f, "Whisper API returned {status}: {message}")
            }
            WhisperError::Json(e) => write!(f, "failed to parse Whisper API response: {e}"),
        }
    }
}

impl std::error::Error for WhisperError {}

impl From<std::io::Error> for WhisperError {
    fn from(e: std::io::Error) -> Self {
        WhisperError::Io(e)
    }
}

#[derive(Deserialize)]
struct ApiErrorBody {
    error: ApiErrorDetail,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: String,
}

pub fn transcribe(
    base_url: &str,
    api_key: &str,
    audio_path: &Path,
) -> Result<Transcript, WhisperError> {
    let audio_bytes = std::fs::read(audio_path)?;
    let filename = audio_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.mp3");
    let filename = sanitize_filename(filename);

    let boundary = "----ai-vedit-boundary-7f3a9c";
    let body = build_multipart_body(boundary, &audio_bytes, &filename);
    let content_type = format!("multipart/form-data; boundary={boundary}");
    let url = format!("{base_url}/v1/audio/transcriptions");

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(300))
        .build();

    let result = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", &content_type)
        .send_bytes(&body);

    let response = match result {
        Ok(response) => response,
        Err(ureq::Error::Status(status, response)) => {
            let body_text = response.into_string().unwrap_or_default();
            let message = serde_json::from_str::<ApiErrorBody>(&body_text)
                .map(|b| b.error.message)
                .unwrap_or(body_text);
            return Err(WhisperError::Api { status, message });
        }
        Err(e) => return Err(WhisperError::Http(e.to_string())),
    };

    let body_text = response.into_string()?;
    serde_json::from_str::<Transcript>(&body_text).map_err(WhisperError::Json)
}

fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| match c {
            '"' | '\r' | '\n' => '_',
            other => other,
        })
        .collect()
}

fn build_multipart_body(boundary: &str, audio_bytes: &[u8], filename: &str) -> Vec<u8> {
    let mut body = Vec::new();

    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"model\"\r\n\r\n");
    body.extend_from_slice(b"whisper-1\r\n");

    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"response_format\"\r\n\r\n");
    body.extend_from_slice(b"verbose_json\r\n");

    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: audio/mpeg\r\n\r\n");
    body.extend_from_slice(audio_bytes);
    body.extend_from_slice(b"\r\n");

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcribe_parses_successful_response() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/audio/transcriptions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"text":"hello world","segments":[{"start":0.0,"end":1.5,"text":"hello world"}]}"#,
            )
            .create();

        let audio_path = std::env::temp_dir().join("ai-vedit-test-audio-1.mp3");
        std::fs::write(&audio_path, b"fake audio bytes").unwrap();

        let result = transcribe(&server.url(), "test-key", &audio_path);

        std::fs::remove_file(&audio_path).ok();

        let transcript = result.expect("expected successful transcription");
        assert_eq!(transcript.text, "hello world");
        assert_eq!(transcript.segments.len(), 1);
        assert_eq!(transcript.segments[0].start, 0.0);
        assert_eq!(transcript.segments[0].end, 1.5);
    }

    #[test]
    fn transcribe_surfaces_api_error_message() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/audio/transcriptions")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":{"message":"Invalid API key"}}"#)
            .create();

        let audio_path = std::env::temp_dir().join("ai-vedit-test-audio-2.mp3");
        std::fs::write(&audio_path, b"fake audio bytes").unwrap();

        let result = transcribe(&server.url(), "bad-key", &audio_path);

        std::fs::remove_file(&audio_path).ok();

        match result {
            Err(WhisperError::Api { status, message }) => {
                assert_eq!(status, 401);
                assert_eq!(message, "Invalid API key");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }

    #[test]
    fn transcribe_sends_correct_multipart_fields() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/audio/transcriptions")
            .match_body(mockito::Matcher::AllOf(vec![
                mockito::Matcher::Regex(r#"name="model""#.to_string()),
                mockito::Matcher::Regex("whisper-1".to_string()),
                mockito::Matcher::Regex(r#"name="response_format""#.to_string()),
                mockito::Matcher::Regex("verbose_json".to_string()),
                mockito::Matcher::Regex(r#"name="file""#.to_string()),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"text":"hi","segments":[]}"#)
            .create();

        let audio_path = std::env::temp_dir().join("ai-vedit-test-audio-multipart.mp3");
        std::fs::write(&audio_path, b"fake audio bytes").unwrap();

        let result = transcribe(&server.url(), "test-key", &audio_path);

        std::fs::remove_file(&audio_path).ok();

        assert!(
            result.is_ok(),
            "expected successful transcription, got {result:?}"
        );
    }

    #[test]
    fn transcribe_falls_back_to_raw_body_on_unparseable_error() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/audio/transcriptions")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let audio_path = std::env::temp_dir().join("ai-vedit-test-audio-3.mp3");
        std::fs::write(&audio_path, b"fake audio bytes").unwrap();

        let result = transcribe(&server.url(), "test-key", &audio_path);

        std::fs::remove_file(&audio_path).ok();

        match result {
            Err(WhisperError::Api { status, message }) => {
                assert_eq!(status, 500);
                assert_eq!(message, "Internal Server Error");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }
}
