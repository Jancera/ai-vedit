use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::library::discover_categories;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetKind {
    Image,
    Video,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub path: PathBuf,
    pub kind: AssetKind,
}

pub fn discover_assets(category_dir: &Path) -> Result<Vec<Asset>, std::io::Error> {
    let entries = match std::fs::read_dir(category_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let mut assets = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !std::fs::metadata(&path)?.is_file() {
            continue;
        }
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
pub struct Selection {
    pub asset: Asset,
    pub used_fallback: bool,
}

pub(crate) fn normalize_category(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Minimal SplitMix64 PRNG — enough to shuffle a category's asset list,
/// not cryptographic. Avoids pulling in the `rand` crate for this one use.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// In-place Fisher-Yates shuffle. Modulo bias is negligible at the
    /// list sizes an asset category realistically holds.
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = (self.next_u64() % (i as u64 + 1)) as usize;
            items.swap(i, j);
        }
    }
}

/// A best-effort non-deterministic seed for the shuffle: wall-clock
/// nanoseconds mixed with the process id, so re-rendering the same plan
/// picks a different asset order each run.
fn entropy_seed() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (std::process::id() as u64).rotate_left(32)
}

pub struct AssetSelector {
    categories: HashMap<String, Vec<Asset>>,
    cursors: HashMap<String, usize>,
    rng: SplitMix64,
}

#[derive(Debug)]
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

impl AssetSelector {
    pub fn new(assets_dir: &Path) -> Result<Self, std::io::Error> {
        Self::with_seed(assets_dir, entropy_seed())
    }

    /// Like [`AssetSelector::new`] but with a caller-provided shuffle seed,
    /// so tests observe a deterministic asset order.
    pub fn with_seed(assets_dir: &Path, seed: u64) -> Result<Self, std::io::Error> {
        let category_names = discover_categories(assets_dir)?;
        let mut rng = SplitMix64::new(seed);

        let mut categories = HashMap::new();
        for name in category_names {
            let mut assets = discover_assets(&assets_dir.join(&name))?;
            rng.shuffle(&mut assets);
            categories.insert(normalize_category(&name), assets);
        }

        Ok(AssetSelector {
            categories,
            cursors: HashMap::new(),
            rng,
        })
    }

    pub fn select(&mut self, category: &str) -> Result<Selection, SelectorError> {
        let requested = normalize_category(category);

        let effective_category = match self.categories.get(&requested) {
            Some(assets) if !assets.is_empty() => requested.clone(),
            _ => match self.categories.get("general") {
                Some(assets) if !assets.is_empty() => "general".to_string(),
                _ => {
                    return Err(SelectorError::EmptyLibrary {
                        category: category.to_string(),
                    })
                }
            },
        };

        let used_fallback = effective_category != requested;

        let assets = self
            .categories
            .get_mut(&effective_category)
            .expect("effective category presence is checked above");
        let cursor = self.cursors.entry(effective_category.clone()).or_insert(0);
        let asset = assets[*cursor].clone();
        *cursor += 1;
        if *cursor >= assets.len() {
            // Bag exhausted — reshuffle so the next pass through this
            // category runs in a fresh random order.
            self.rng.shuffle(assets);
            *cursor = 0;
        }

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

    #[test]
    fn discover_assets_follows_symlinked_files() {
        let dir = tempfile::tempdir().unwrap();
        let real_file = dir.path().join("real.jpg");
        std::fs::write(&real_file, b"").unwrap();
        std::os::unix::fs::symlink(&real_file, dir.path().join("linked.jpg")).unwrap();

        let mut assets = discover_assets(dir.path()).unwrap();
        assets.sort_by(|a, b| a.path.cmp(&b.path));

        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].path, dir.path().join("linked.jpg"));
        assert_eq!(assets[1].path, dir.path().join("real.jpg"));
    }
}

#[cfg(test)]
mod selector_tests {
    use super::*;
    use std::path::Path;

    fn write_asset(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"").unwrap();
    }

    fn make_category(dir: &Path, name: &str, count: usize) -> Vec<PathBuf> {
        std::fs::create_dir(dir.join(name)).unwrap();
        (0..count)
            .map(|i| {
                let file = format!("asset-{i:02}.jpg");
                write_asset(&dir.join(name), &file);
                dir.join(name).join(file)
            })
            .collect()
    }

    #[test]
    fn select_uses_every_asset_once_per_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let mut expected = make_category(dir.path(), "city-broll", 12);
        expected.sort();

        let mut selector = AssetSelector::with_seed(dir.path(), 1).unwrap();

        let mut seen: Vec<PathBuf> = (0..12)
            .map(|_| selector.select("city-broll").unwrap().asset.path)
            .collect();
        seen.sort();

        assert_eq!(seen, expected, "one full cycle should hit every asset once");
    }

    #[test]
    fn select_reshuffles_after_exhausting_the_bag() {
        let dir = tempfile::tempdir().unwrap();
        make_category(dir.path(), "city-broll", 12);

        let mut selector = AssetSelector::with_seed(dir.path(), 7).unwrap();

        let cycle: Vec<PathBuf> = (0..12)
            .map(|_| selector.select("city-broll").unwrap().asset.path)
            .collect();
        let next_cycle: Vec<PathBuf> = (0..12)
            .map(|_| selector.select("city-broll").unwrap().asset.path)
            .collect();

        let mut sorted_next = next_cycle.clone();
        sorted_next.sort();
        let mut sorted_first = cycle.clone();
        sorted_first.sort();
        assert_eq!(sorted_first, sorted_next, "each cycle covers the full set");
        assert_ne!(
            cycle, next_cycle,
            "the bag should be reshuffled into a new order after exhaustion"
        );
    }

    #[test]
    fn select_order_depends_on_seed() {
        let dir = tempfile::tempdir().unwrap();
        make_category(dir.path(), "city-broll", 12);

        let mut a = AssetSelector::with_seed(dir.path(), 1).unwrap();
        let mut b = AssetSelector::with_seed(dir.path(), 2).unwrap();

        let order_a: Vec<PathBuf> = (0..12)
            .map(|_| a.select("city-broll").unwrap().asset.path)
            .collect();
        let order_b: Vec<PathBuf> = (0..12)
            .map(|_| b.select("city-broll").unwrap().asset.path)
            .collect();

        assert_ne!(
            order_a, order_b,
            "different seeds should shuffle differently"
        );
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

    #[test]
    fn select_matches_category_case_insensitively_and_trims_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("city-broll")).unwrap();
        write_asset(&dir.path().join("city-broll"), "a.jpg");

        let mut selector = AssetSelector::new(dir.path()).unwrap();

        let selection = selector.select("  City-Broll  ").unwrap();

        assert_eq!(
            selection.asset.path,
            dir.path().join("city-broll").join("a.jpg")
        );
        assert!(!selection.used_fallback);
    }
}
