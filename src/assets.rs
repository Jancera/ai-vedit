use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AssetKind {
    Image,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Asset {
    pub path: PathBuf,
    pub kind: AssetKind,
}

#[allow(dead_code)]
pub fn discover_assets(category_dir: &Path) -> Result<Vec<Asset>, std::io::Error> {
    let entries = match std::fs::read_dir(category_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let mut assets = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if let Some(kind) = classify_extension(&path) {
            assets.push(Asset { path, kind });
        }
    }

    assets.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(assets)
}

fn classify_extension(path: &Path) -> Option<AssetKind> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "webp" => Some(AssetKind::Image),
        "mp4" => Some(AssetKind::Video),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_assets_filters_and_classifies_supported_types() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("photo.jpg"), b"").unwrap();
        std::fs::write(dir.path().join("photo2.PNG"), b"").unwrap();
        std::fs::write(dir.path().join("clip.mp4"), b"").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"").unwrap();

        let assets = discover_assets(dir.path()).unwrap();

        assert_eq!(assets.len(), 3);
        assert_eq!(assets[0].path, dir.path().join("clip.mp4"));
        assert_eq!(assets[0].kind, AssetKind::Video);
        assert_eq!(assets[1].path, dir.path().join("photo.jpg"));
        assert_eq!(assets[1].kind, AssetKind::Image);
        assert_eq!(assets[2].path, dir.path().join("photo2.PNG"));
        assert_eq!(assets[2].kind, AssetKind::Image);
    }

    #[test]
    fn discover_assets_returns_empty_for_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");

        let assets = discover_assets(&missing).unwrap();

        assert!(assets.is_empty());
    }

    #[test]
    fn discover_assets_returns_empty_for_empty_dir() {
        let dir = tempfile::tempdir().unwrap();

        let assets = discover_assets(dir.path()).unwrap();

        assert!(assets.is_empty());
    }
}
