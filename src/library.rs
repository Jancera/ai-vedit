use std::path::Path;

pub fn discover_categories(assets_dir: &Path) -> Result<Vec<String>, std::io::Error> {
    let entries = match std::fs::read_dir(assets_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };

    let mut categories = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                categories.push(name.to_string());
            }
        }
    }

    Ok(categories)
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
}
