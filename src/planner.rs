use std::fmt;

use serde::{Deserialize, Serialize};

use crate::whisper::Transcript;

const MODEL: &str = "gpt-4o-mini";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Beat {
    pub start: f64,
    pub end: f64,
    /// Derived as `end - start`, not requested from the model. Populated by
    /// `plan_beats` after parsing; defaults to 0.0 so a beat freshly
    /// deserialized from the model's response (which never includes this
    /// field) doesn't fail to parse before that happens.
    #[serde(default)]
    pub duration: f64,
    pub description: String,
    pub category: String,
    pub is_new_category: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub beats: Vec<Beat>,
}

#[derive(Debug)]
pub enum PlannerError {
    Http(String),
    Api { status: u16, message: String },
    Json(serde_json::Error),
    EmptyResponse,
}

impl fmt::Display for PlannerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlannerError::Http(e) => write!(f, "request to planning API failed: {e}"),
            PlannerError::Api { status, message } => {
                write!(f, "planning API returned {status}: {message}")
            }
            PlannerError::Json(e) => write!(f, "failed to parse planning API response: {e}"),
            PlannerError::EmptyResponse => {
                write!(f, "planning API returned no beats for this transcript")
            }
        }
    }
}

impl std::error::Error for PlannerError {}

#[derive(Deserialize)]
struct ApiErrorBody {
    error: ApiErrorDetail,
}

#[derive(Deserialize)]
struct ApiErrorDetail {
    message: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

pub fn plan_beats(
    base_url: &str,
    api_key: &str,
    transcript: &Transcript,
    existing_categories: &[String],
) -> Result<Plan, PlannerError> {
    let url = format!("{base_url}/v1/chat/completions");
    let body = build_request_body(transcript, existing_categories);

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(120))
        .build();

    let result = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string());

    let response = match result {
        Ok(response) => response,
        Err(ureq::Error::Status(status, response)) => {
            let body_text = response.into_string().unwrap_or_default();
            let message = serde_json::from_str::<ApiErrorBody>(&body_text)
                .map(|b| b.error.message)
                .unwrap_or(body_text);
            return Err(PlannerError::Api { status, message });
        }
        Err(e) => return Err(PlannerError::Http(e.to_string())),
    };

    let body_text = response
        .into_string()
        .map_err(|e| PlannerError::Http(e.to_string()))?;
    let chat_response =
        serde_json::from_str::<ChatResponse>(&body_text).map_err(PlannerError::Json)?;

    let content = chat_response
        .choices
        .first()
        .map(|c| c.message.content.as_str())
        .ok_or(PlannerError::EmptyResponse)?;

    let mut plan = serde_json::from_str::<Plan>(content).map_err(PlannerError::Json)?;

    if plan.beats.is_empty() {
        return Err(PlannerError::EmptyResponse);
    }

    for beat in &mut plan.beats {
        beat.duration = beat.end - beat.start;
    }

    Ok(plan)
}

fn build_request_body(
    transcript: &Transcript,
    existing_categories: &[String],
) -> serde_json::Value {
    let system_prompt = "You segment narration transcripts into short narrative beats for a \
        video editor. For each beat, provide a start/end time (seconds, matching the supplied \
        transcript segment timestamps), a short description, and an asset category. \
        \n\nFor each beat, identify the distinct visual subject implied by that beat's content \
        (e.g. a specific place, object, action, or concept) and choose a short, kebab-case \
        category name that names that subject. Beats about different subjects should usually \
        get different categories \u{2014} do not default every beat to the same category. \
        \n\nexisting_categories lists category names already in use; reuse one only when it is \
        a genuine match for the beat's subject, not merely because it already exists \u{2014} \
        it is there to help you avoid creating a near-duplicate of a category that already \
        covers the same subject, not to limit your choices. When no existing category is a \
        good match, propose a new one and set is_new_category to true. \
        \n\nAvoid generic categories like \"general\" or \"narration\": only use a generic \
        category for a beat that truly has no distinct visual subject (e.g. a pure transition \
        or a beat that is just restating something already covered by a nearby beat).";

    let user_content = serde_json::json!({
        "transcript_text": transcript.text,
        "segments": transcript.segments,
        "existing_categories": existing_categories,
    });

    serde_json::json!({
        "model": MODEL,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_content.to_string()},
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "plan",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "beats": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "start": {"type": "number"},
                                    "end": {"type": "number"},
                                    "description": {"type": "string"},
                                    "category": {"type": "string"},
                                    "is_new_category": {"type": "boolean"},
                                },
                                "required": ["start", "end", "description", "category", "is_new_category"],
                                "additionalProperties": false,
                            }
                        }
                    },
                    "required": ["beats"],
                    "additionalProperties": false,
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::whisper::{Segment, Transcript};

    fn sample_transcript() -> Transcript {
        Transcript {
            text: "Here is a city at night. Then a product shot.".to_string(),
            segments: vec![
                Segment {
                    start: 0.0,
                    end: 3.0,
                    text: "Here is a city at night.".to_string(),
                },
                Segment {
                    start: 3.0,
                    end: 6.0,
                    text: "Then a product shot.".to_string(),
                },
            ],
            duration: 6.0,
        }
    }

    fn structured_response_body() -> String {
        let content = r#"{"beats":[
            {"start":0.0,"end":3.0,"description":"City at night","category":"city-broll","is_new_category":false},
            {"start":3.0,"end":6.0,"description":"Product shot","category":"product-shots","is_new_category":true}
        ]}"#;
        format!(
            r#"{{"choices":[{{"message":{{"content":{}}}}}]}}"#,
            serde_json::to_string(content).unwrap()
        )
    }

    #[test]
    fn build_request_body_discourages_generic_catch_all_categories() {
        let transcript = sample_transcript();
        let categories = vec!["general".to_string()];

        let body = build_request_body(&transcript, &categories);
        let system_prompt = body["messages"][0]["content"].as_str().unwrap();

        assert!(system_prompt.contains("Avoid generic categories"));
        assert!(system_prompt.contains("not merely because it already exists"));
    }

    #[test]
    fn plan_beats_parses_successful_response() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(structured_response_body())
            .create();

        let transcript = sample_transcript();
        let categories = vec!["city-broll".to_string()];

        let plan = plan_beats(&server.url(), "test-key", &transcript, &categories)
            .expect("expected successful plan");

        assert_eq!(plan.beats.len(), 2);
        assert_eq!(plan.beats[0].category, "city-broll");
        assert_eq!(plan.beats[0].duration, 3.0);
        assert!(!plan.beats[0].is_new_category);
        assert_eq!(plan.beats[1].category, "product-shots");
        assert_eq!(plan.beats[1].duration, 3.0);
        assert!(plan.beats[1].is_new_category);
    }

    #[test]
    fn plan_beats_surfaces_api_error_message() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":{"message":"Invalid API key"}}"#)
            .create();

        let transcript = sample_transcript();
        let result = plan_beats(&server.url(), "bad-key", &transcript, &[]);

        match result {
            Err(PlannerError::Api { status, message }) => {
                assert_eq!(status, 401);
                assert_eq!(message, "Invalid API key");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }

    #[test]
    fn plan_beats_falls_back_to_raw_body_on_unparseable_error() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(500)
            .with_body("Internal Server Error")
            .create();

        let transcript = sample_transcript();
        let result = plan_beats(&server.url(), "test-key", &transcript, &[]);

        match result {
            Err(PlannerError::Api { status, message }) => {
                assert_eq!(status, 500);
                assert_eq!(message, "Internal Server Error");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }

    #[test]
    fn plan_beats_rejects_empty_beat_list() {
        let mut server = mockito::Server::new();
        let content = r#"{"beats":[]}"#;
        let body = format!(
            r#"{{"choices":[{{"message":{{"content":{}}}}}]}}"#,
            serde_json::to_string(content).unwrap()
        );
        let _mock = server
            .mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create();

        let transcript = sample_transcript();
        let result = plan_beats(&server.url(), "test-key", &transcript, &[]);

        match result {
            Err(PlannerError::EmptyResponse) => {}
            other => panic!("expected EmptyResponse, got {other:?}"),
        }
    }
}
