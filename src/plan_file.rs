use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cli::AspectRatio;
use crate::planner::Beat;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanFile {
    pub audio_path: PathBuf,
    pub aspect: AspectRatio,
    pub beats: Vec<Beat>,
}

// TODO(M4): remove once render calls load
#[allow(dead_code)]
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

// TODO(M4): remove once render calls load
#[allow(dead_code)]
pub fn load(path: &Path) -> Result<PlanFile, PlanFileError> {
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(PlanFileError::Json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::AspectRatio;
    use crate::planner::Beat;

    fn sample_plan_file() -> PlanFile {
        PlanFile {
            audio_path: "script.mp3".into(),
            aspect: AspectRatio::Sixteen9,
            beats: vec![Beat {
                start: 0.0,
                end: 3.0,
                description: "City at night".to_string(),
                category: "city-broll".to_string(),
                is_new_category: false,
            }],
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
}
