use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::planner::Beat;
use crate::subtitles::Subtitles;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanFile {
    pub audio_path: PathBuf,
    pub beats: Vec<Beat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitles: Option<Subtitles>,
}

#[derive(Debug)]
pub enum PlanFileError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for PlanFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanFileError::Io(e) => write!(f, "failed to read plan file: {e}"),
            PlanFileError::Json(e) => write!(f, "failed to parse plan file: {e}"),
        }
    }
}

impl std::error::Error for PlanFileError {}

impl From<std::io::Error> for PlanFileError {
    fn from(e: std::io::Error) -> Self {
        PlanFileError::Io(e)
    }
}

pub fn save(path: &Path, plan_file: &PlanFile) -> Result<(), std::io::Error> {
    let content = serde_json::to_string_pretty(plan_file).map_err(std::io::Error::other)?;
    std::fs::write(path, content)
}

pub fn load(path: &Path) -> Result<PlanFile, PlanFileError> {
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(PlanFileError::Json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::Beat;
    use crate::subtitles::{Cue, SubtitleStyle, Subtitles};

    fn sample_plan_file() -> PlanFile {
        PlanFile {
            audio_path: "script.mp3".into(),
            beats: vec![Beat {
                start: 0.0,
                end: 3.0,
                duration: 3.0,
                description: "City at night".to_string(),
                category: "city-broll".to_string(),
                is_new_category: false,
            }],
            subtitles: None,
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plan.json");
        let plan_file = sample_plan_file();

        save(&path, &plan_file).unwrap();
        let loaded = load(&path).expect("expected plan file to load");

        assert_eq!(loaded, plan_file);
    }

    #[test]
    fn load_accepts_plan_without_aspect_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plan.json");
        std::fs::write(&path, r#"{"audio_path":"script.mp3","beats":[]}"#).unwrap();

        let loaded = load(&path).expect("a plan without an aspect field should load");

        assert_eq!(loaded.audio_path, PathBuf::from("script.mp3"));
        assert!(loaded.beats.is_empty());
    }

    #[test]
    fn load_ignores_legacy_aspect_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plan.json");
        std::fs::write(
            &path,
            r#"{"audio_path":"script.mp3","aspect":"Sixteen9","beats":[]}"#,
        )
        .unwrap();

        load(&path).expect("a legacy plan carrying an aspect field should still load");
    }

    #[test]
    fn load_returns_error_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nonexistent.json");

        match load(&missing) {
            Err(PlanFileError::Io(_)) => {}
            other => panic!("expected Io error, got {other:?}"),
        }
    }

    #[test]
    fn load_returns_error_for_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.json");
        std::fs::write(&path, b"not valid json").unwrap();

        match load(&path) {
            Err(PlanFileError::Json(_)) => {}
            other => panic!("expected Json error, got {other:?}"),
        }
    }

    #[test]
    fn save_omits_subtitles_key_when_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plan.json");
        save(&path, &sample_plan_file()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let plan: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(
            plan.get("subtitles").is_none(),
            "plan.json must not carry a subtitles key when None: {text}"
        );
    }

    #[test]
    fn save_then_load_round_trips_with_populated_subtitles() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plan.json");
        let mut plan_file = sample_plan_file();
        plan_file.subtitles = Some(Subtitles {
            enabled: true,
            style: SubtitleStyle::default(),
            cues: vec![Cue {
                start: 0.0,
                end: 2.0,
                text: "hi".into(),
            }],
        });

        save(&path, &plan_file).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\"subtitles\""),
            "serialized plan should carry a subtitles key: {text}"
        );

        let loaded = load(&path).expect("expected plan file to load");
        assert_eq!(loaded, plan_file);
    }
}
