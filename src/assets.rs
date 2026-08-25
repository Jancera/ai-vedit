use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::library::discover_categories;

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

#[derive(Debug)]
#[allow(dead_code)]
pub struct Selection {
    pub asset: Asset,
    pub used_fallback: bool,
}

#[allow(dead_code)]
pub struct AssetSelector {
    categories: HashMap<String, Vec<Asset>>,
    cursors: HashMap<String, usize>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum SelectorError {
    Io(std::io::Error),
    EmptyLibrary { category: String },
}

impl fmt::Display for SelectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelectorError::Io(e) => write!(f, "failed to read asset library: {e}"),
            SelectorError::EmptyLibrary { category } => {
                write!(
                    f,
                    "category {category:?} has no assets, and the general/ fallback is also empty"
                )
            }
        }
    }
}

impl std::error::Error for SelectorError {}

impl From<std::io::Error> for SelectorError {
    fn from(e: std::io::Error) -> Self {
        SelectorError::Io(e)
    }
}

#[allow(dead_code)]
impl AssetSelector {
    pub fn new(assets_dir: &Path) -> Result<Self, std::io::Error> {
        let category_names = discover_categories(assets_dir)?;

        let mut categories = HashMap::new();
        for name in category_names {
            let assets = discover_assets(&assets_dir.join(&name))?;
            categories.insert(name, assets);
        }

        Ok(AssetSelector {
            categories,
            cursors: HashMap::new(),
        })
    }

    pub fn select(&mut self, category: &str) -> Result<Selection, SelectorError> {
        let effective_category = match self.categories.get(category) {
            Some(assets) if !assets.is_empty() => category,
            _ => match self.categories.get("general") {
                Some(assets) if !assets.is_empty() => "general",
                _ => {
                    return Err(SelectorError::EmptyLibrary {
                        category: category.to_string(),
                    })
                }
            },
        };

        let used_fallback = effective_category != category;

        let assets = &self.categories[effective_category];
        let cursor = self
            .cursors
            .entry(effective_category.to_string())
            .or_insert(0);
        let asset = assets[*cursor % assets.len()].clone();
        *cursor += 1;

        Ok(Selection {
            asset,
            used_fallback,
        })
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

#[cfg(test)]
mod selector_tests {
    use super::*;
    use std::path::Path;

    fn write_asset(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"").unwrap();
    }

    #[test]
    fn select_cycles_round_robin_within_a_category() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("city-broll")).unwrap();
        write_asset(&dir.path().join("city-broll"), "a.jpg");
        write_asset(&dir.path().join("city-broll"), "b.jpg");

        let mut selector = AssetSelector::new(dir.path()).unwrap();

        let first = selector.select("city-broll").unwrap();
        let second = selector.select("city-broll").unwrap();
        let third = selector.select("city-broll").unwrap();

        assert_eq!(
            first.asset.path,
            dir.path().join("city-broll").join("a.jpg")
        );
        assert!(!first.used_fallback);
        assert_eq!(
            second.asset.path,
            dir.path().join("city-broll").join("b.jpg")
        );
        assert!(!second.used_fallback);
        assert_eq!(
            third.asset.path,
            dir.path().join("city-broll").join("a.jpg")
        );
        assert!(!third.used_fallback);
    }

    #[test]
    fn select_falls_back_to_general_when_category_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("city-broll")).unwrap();
        std::fs::create_dir(dir.path().join("general")).unwrap();
        write_asset(&dir.path().join("general"), "filler.mp4");

        let mut selector = AssetSelector::new(dir.path()).unwrap();

        let selection = selector.select("city-broll").unwrap();

        assert_eq!(
            selection.asset.path,
            dir.path().join("general").join("filler.mp4")
        );
        assert!(selection.used_fallback);
    }

    #[test]
    fn select_falls_back_to_general_for_unknown_category() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("general")).unwrap();
        write_asset(&dir.path().join("general"), "filler.mp4");

        let mut selector = AssetSelector::new(dir.path()).unwrap();

        let selection = selector.select("does-not-exist").unwrap();

        assert_eq!(
            selection.asset.path,
            dir.path().join("general").join("filler.mp4")
        );
        assert!(selection.used_fallback);
    }

    #[test]
    fn select_errors_when_category_and_general_are_both_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("city-broll")).unwrap();

        let mut selector = AssetSelector::new(dir.path()).unwrap();

        match selector.select("city-broll") {
            Err(SelectorError::EmptyLibrary { category }) => {
                assert_eq!(category, "city-broll");
            }
            other => panic!("expected EmptyLibrary error, got {other:?}"),
        }
    }

    #[test]
    fn select_errors_when_general_exists_but_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("city-broll")).unwrap();
        std::fs::create_dir(dir.path().join("general")).unwrap();

        let mut selector = AssetSelector::new(dir.path()).unwrap();

        match selector.select("city-broll") {
            Err(SelectorError::EmptyLibrary { category }) => {
                assert_eq!(category, "city-broll");
            }
            other => panic!("expected EmptyLibrary error, got {other:?}"),
        }
    }
}
