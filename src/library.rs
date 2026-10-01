use std::collections::BTreeSet;
use std::path::Path;

use crate::assets::normalize_category;

pub fn discover_categories(assets_dir: &Path) -> Result<Vec<String>, std::io::Error> {
    let entries = match std::fs::read_dir(assets_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let mut categories = Vec::new();
    for entry in entries {
        let entry = entry?;
        if std::fs::metadata(entry.path())?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                categories.push(name.to_string());
            }
        }
    }

    Ok(categories)
}

/// Ensures the assets directory exists, creating it (and any missing
/// parents) if needed. Returns whether it was freshly created.
pub fn ensure_assets_dir(assets_dir: &Path) -> Result<bool, std::io::Error> {
    if assets_dir.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(assets_dir)?;
    Ok(true)
}

/// Ensures a subdirectory exists under `assets_dir` for each category name,
/// after normalizing (trim + lowercase) and deduplicating them the same way
/// `AssetSelector` looks categories up. Returns the normalized names of the
/// directories that were freshly created; categories that already had a
/// directory are silently skipped.
pub fn ensure_category_dirs<'a>(
    assets_dir: &Path,
    categories: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<String>, std::io::Error> {
    let normalized: BTreeSet<String> = categories.into_iter().map(normalize_category).collect();

    let mut created = Vec::new();
    for category in normalized {
        let dir = assets_dir.join(&category);
        if dir.exists() {
            continue;
        }
        std::fs::create_dir_all(&dir)?;
        created.push(category);
    }

    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_categories_lists_only_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("city-broll")).unwrap();
        std::fs::create_dir(dir.path().join("product-shots")).unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"not a category").unwrap();

        let mut categories = discover_categories(dir.path()).unwrap();
        categories.sort();

        assert_eq!(categories, vec!["city-broll", "product-shots"]);
    }

    #[test]
    fn discover_categories_returns_empty_for_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");

        let categories = discover_categories(&missing).unwrap();

        assert!(categories.is_empty());
    }

    #[test]
    fn discover_categories_returns_empty_for_empty_dir() {
        let dir = tempfile::tempdir().unwrap();

        let categories = discover_categories(dir.path()).unwrap();

        assert!(categories.is_empty());
    }

    #[test]
    fn discover_categories_follows_symlinked_directories() {
        let dir = tempfile::tempdir().unwrap();
        let real_dir = dir.path().join("real-category");
        std::fs::create_dir(&real_dir).unwrap();
        std::os::unix::fs::symlink(&real_dir, dir.path().join("linked-category")).unwrap();

        let mut categories = discover_categories(dir.path()).unwrap();
        categories.sort();

        assert_eq!(categories, vec!["linked-category", "real-category"]);
    }

    #[test]
    fn ensure_assets_dir_creates_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let assets_dir = dir.path().join("assets");

        let created = ensure_assets_dir(&assets_dir).unwrap();

        assert!(created);
        assert!(assets_dir.is_dir());
    }

    #[test]
    fn ensure_assets_dir_reports_false_when_already_present() {
        let dir = tempfile::tempdir().unwrap();

        let created = ensure_assets_dir(dir.path()).unwrap();

        assert!(!created);
        assert!(dir.path().is_dir());
    }

    #[test]
    fn ensure_category_dirs_creates_only_missing_categories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("city-broll")).unwrap();

        let mut created =
            ensure_category_dirs(dir.path(), ["city-broll", "product-shots"]).unwrap();
        created.sort();

        assert_eq!(created, vec!["product-shots"]);
        assert!(dir.path().join("city-broll").is_dir());
        assert!(dir.path().join("product-shots").is_dir());
    }

    #[test]
    fn ensure_category_dirs_normalizes_and_dedupes_names() {
        let dir = tempfile::tempdir().unwrap();

        let created =
            ensure_category_dirs(dir.path(), ["City-Broll", " city-broll ", "CITY-BROLL"]).unwrap();

        assert_eq!(created, vec!["city-broll".to_string()]);
        assert!(dir.path().join("city-broll").is_dir());
    }

    #[test]
    fn ensure_category_dirs_returns_empty_when_all_already_exist() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("general")).unwrap();

        let created = ensure_category_dirs(dir.path(), ["general"]).unwrap();

        assert!(created.is_empty());
    }
}
