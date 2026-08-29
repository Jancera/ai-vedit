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
    min_beat_duration: f64,
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

    plan.beats = normalize_plan(plan.beats, min_beat_duration, transcript.duration);

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

fn group_duration(group: &[Beat]) -> f64 {
    group.iter().map(|b| b.duration).sum()
}

fn collapse_group(group: Vec<Beat>) -> Beat {
    let duration = group_duration(&group);
    let start = group.first().map(|b| b.start).unwrap_or(0.0);
    let end = group.last().map(|b| b.end).unwrap_or(0.0);
    let representative = group
        .into_iter()
        .max_by(|a, b| {
            a.duration
                .partial_cmp(&b.duration)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("merge group is never empty");

    Beat {
        start,
        end,
        duration,
        description: representative.description,
        category: representative.category,
        is_new_category: representative.is_new_category,
    }
}

/// Collapses consecutive beats into groups whose summed duration is at
/// least `min_duration`, one merged `Beat` per group. A trailing group
/// that never reaches `min_duration` (ran out of beats) is folded
/// backward into the previous group instead of being emitted short.
/// `start`/`end` on the returned beats are only a rough first pass
/// (first sub-beat's start, last sub-beat's end) — `relayout_beats`
/// (Task 4) is what makes them authoritative.
fn merge_short_beats(beats: Vec<Beat>, min_duration: f64) -> Vec<Beat> {
    let mut groups: Vec<Vec<Beat>> = Vec::new();

    for beat in beats {
        match groups.last_mut() {
            Some(group) if group_duration(group) < min_duration => group.push(beat),
            _ => groups.push(vec![beat]),
        }
    }

    if groups.len() > 1 {
        let trailing_is_short = group_duration(groups.last().unwrap()) < min_duration;
        if trailing_is_short {
            let trailing = groups.pop().unwrap();
            groups.last_mut().unwrap().extend(trailing);
        }
    }

    groups.into_iter().map(collapse_group).collect()
}

/// Lays `start`/`end` out as one contiguous timeline starting at 0, purely
/// from each beat's `duration` — the model's own start/end values are
/// never trusted for this, only relative durations are.
fn relayout_beats(beats: &mut [Beat]) {
    let mut cursor = 0.0;
    for beat in beats.iter_mut() {
        beat.start = cursor;
        beat.end = cursor + beat.duration;
        cursor = beat.end;
    }
}

/// Merges short beats, then (when `total_duration` is known) proportionally
/// rescales every beat's duration so the beats' combined length matches it
/// exactly, then relays out `start`/`end` as a gap-free timeline. When
/// `total_duration` is `<= 0.0` (unknown — e.g. a transcript cached before
/// `Transcript.duration` existed), the rescale step is skipped and only the
/// merge + relayout apply.
fn normalize_plan(beats: Vec<Beat>, min_beat_duration: f64, total_duration: f64) -> Vec<Beat> {
    let mut beats = merge_short_beats(beats, min_beat_duration);

    if total_duration > 0.0 {
        let current_total: f64 = beats.iter().map(|b| b.duration).sum();
        if current_total > 0.0 {
            let scale = total_duration / current_total;
            for beat in &mut beats {
                beat.duration *= scale;
            }
        }
    }

    relayout_beats(&mut beats);

    beats
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

    fn beat_with_duration(duration: f64, category: &str) -> Beat {
        Beat {
            start: 0.0,
            end: duration,
            duration,
            description: category.to_string(),
            category: category.to_string(),
            is_new_category: false,
        }
    }

    #[test]
    fn merge_short_beats_returns_empty_for_no_beats() {
        let merged = merge_short_beats(Vec::new(), 5.0);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_short_beats_leaves_beats_alone_when_each_meets_minimum() {
        let beats = vec![beat_with_duration(5.0, "a"), beat_with_duration(6.0, "b")];

        let merged = merge_short_beats(beats, 5.0);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].category, "a");
        assert_eq!(merged[1].category, "b");
    }

    #[test]
    fn merge_short_beats_merges_consecutive_short_beats() {
        let beats = vec![
            beat_with_duration(1.0, "a"),
            beat_with_duration(1.0, "b"),
            beat_with_duration(4.0, "c"),
        ];

        let merged = merge_short_beats(beats, 5.0);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].duration, 6.0);
        assert_eq!(merged[0].category, "c");
    }

    #[test]
    fn merge_short_beats_folds_short_trailing_group_backward() {
        let beats = vec![
            beat_with_duration(6.0, "long"),
            beat_with_duration(2.0, "short-tail"),
        ];

        let merged = merge_short_beats(beats, 5.0);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].duration, 8.0);
        assert_eq!(merged[0].category, "long");
    }

    #[test]
    fn merge_short_beats_is_noop_for_zero_minimum() {
        let beats = vec![beat_with_duration(1.0, "a"), beat_with_duration(1.0, "b")];

        let merged = merge_short_beats(beats, 0.0);

        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn normalize_plan_rescales_beats_to_match_a_longer_total_duration() {
        let beats = vec![beat_with_duration(2.0, "a"), beat_with_duration(2.0, "b")];

        let normalized = normalize_plan(beats, 0.0, 8.0);

        assert_eq!(normalized[0].duration, 4.0);
        assert_eq!(normalized[1].duration, 4.0);
    }

    #[test]
    fn normalize_plan_rescales_beats_to_match_a_shorter_total_duration() {
        let beats = vec![beat_with_duration(4.0, "a"), beat_with_duration(4.0, "b")];

        let normalized = normalize_plan(beats, 0.0, 4.0);

        assert_eq!(normalized[0].duration, 2.0);
        assert_eq!(normalized[1].duration, 2.0);
    }

    #[test]
    fn normalize_plan_skips_rescale_when_total_duration_is_unknown() {
        let beats = vec![beat_with_duration(2.0, "a"), beat_with_duration(2.0, "b")];

        let normalized = normalize_plan(beats, 0.0, 0.0);

        assert_eq!(normalized[0].duration, 2.0);
        assert_eq!(normalized[1].duration, 2.0);
    }

    #[test]
    fn normalize_plan_produces_contiguous_start_and_end_with_no_gaps() {
        let beats = vec![beat_with_duration(2.0, "a"), beat_with_duration(3.0, "b")];

        let normalized = normalize_plan(beats, 0.0, 0.0);

        assert_eq!(normalized[0].start, 0.0);
        assert_eq!(normalized[0].end, 2.0);
        assert_eq!(normalized[1].start, 2.0);
        assert_eq!(normalized[1].end, 5.0);
    }

    #[test]
    fn normalize_plan_composes_merge_and_rescale() {
        let beats = vec![
            beat_with_duration(1.0, "a"),
            beat_with_duration(1.0, "b"),
            beat_with_duration(2.0, "c"),
        ];

        let normalized = normalize_plan(beats, 3.0, 8.0);

        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].duration, 8.0);
        assert_eq!(normalized[0].start, 0.0);
        assert_eq!(normalized[0].end, 8.0);
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

        let plan = plan_beats(&server.url(), "test-key", &transcript, &categories, 0.0)
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
        let result = plan_beats(&server.url(), "bad-key", &transcript, &[], 0.0);

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
        let result = plan_beats(&server.url(), "test-key", &transcript, &[], 0.0);

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
        let result = plan_beats(&server.url(), "test-key", &transcript, &[], 0.0);

        match result {
            Err(PlannerError::EmptyResponse) => {}
            other => panic!("expected EmptyResponse, got {other:?}"),
        }
    }
}
