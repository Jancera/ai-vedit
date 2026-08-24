use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::whisper::Transcript;

pub fn cache_path_for(audio_path: &Path) -> Result<PathBuf, std::io::Error> {
    let audio_bytes = fs::read(audio_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&audio_bytes);
    let hash = hasher.finalize();
    let hex_hash = hash.iter().map(|b| format!("{b:02x}")).collect::<String>();

    let dir = audio_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".cache");

    Ok(dir.join(format!("{hex_hash}.json")))
}

pub fn load(cache_path: &Path) -> Option<Transcript> {
    let content = fs::read_to_string(cache_path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save(cache_path: &Path, transcript: &Transcript) -> Result<(), std::io::Error> {
    if let Some(dir) = cache_path.parent() {
        fs::create_dir_all(dir)?;
    }
    let content = serde_json::to_string_pretty(transcript).map_err(std::io::Error::other)?;
    fs::write(cache_path, content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::whisper::{Segment, Transcript};

    fn sample_transcript() -> Transcript {
        Transcript {
            text: "hello".to_string(),
            segments: vec![Segment {
                start: 0.0,
                end: 1.0,
                text: "hello".to_string(),
            }],
        }
    }

    #[test]
    fn cache_path_is_stable_for_same_content() {
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("script.mp3");
        std::fs::write(&audio_path, b"same bytes").unwrap();

        let path1 = cache_path_for(&audio_path).unwrap();
        let path2 = cache_path_for(&audio_path).unwrap();

        assert_eq!(path1, path2);
        assert_eq!(path1.parent().unwrap(), dir.path().join(".cache"));
    }

    #[test]
    fn cache_path_differs_for_different_content() {
        let dir = tempfile::tempdir().unwrap();
        let audio_a = dir.path().join("a.mp3");
        let audio_b = dir.path().join("b.mp3");
        std::fs::write(&audio_a, b"content a").unwrap();
        std::fs::write(&audio_b, b"content b").unwrap();

        let path_a = cache_path_for(&audio_a).unwrap();
        let path_b = cache_path_for(&audio_b).unwrap();

        assert_ne!(path_a, path_b);
    }

    #[test]
    fn load_returns_none_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join(".cache").join("nonexistent.json");

        assert!(load(&missing).is_none());
    }

    #[test]
    fn load_returns_none_for_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join(".cache").join("corrupt.json");
        std::fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        std::fs::write(&cache_path, b"not valid json").unwrap();

        assert!(load(&cache_path).is_none());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join(".cache").join("abc123.json");
        let transcript = sample_transcript();

        save(&cache_path, &transcript).unwrap();
        let loaded = load(&cache_path).expect("expected cached transcript to load");

        assert_eq!(loaded, transcript);
    }
}
