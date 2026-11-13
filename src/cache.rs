//! Incremental compilation fingerprints.
//!
//! A build is fresh when the contents of every `.tdm` file that could
//! contribute to it (the entry point and any module beside it) are unchanged
//! since the previous build.  Fingerprints are stored next to the generated
//! Lua, so `target/` can be deleted to force a rebuild.

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

/// Directories that never participate in a build fingerprint.
const IGNORED_DIRECTORIES: &[&str] = &["target", ".git", ".mlua-pkgs", "node_modules"];

/// Computes a stable fingerprint of every `.tdm` file under `root`.
pub fn fingerprint(root: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    env!("CARGO_PKG_VERSION").hash(&mut hasher);
    let mut files = Vec::new();
    collect_sources(root, &mut files);
    files.sort();
    for file in files {
        file.hash(&mut hasher);
        match fs::read(&file) {
            Ok(contents) => contents.hash(&mut hasher),
            Err(_) => 0u8.hash(&mut hasher),
        }
    }
    hasher.finish()
}

fn collect_sources(directory: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let name = entry.file_name();
            if IGNORED_DIRECTORIES
                .iter()
                .any(|ignored| name.to_string_lossy() == *ignored)
            {
                continue;
            }
            collect_sources(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "tdm") {
            out.push(path);
        }
    }
}

/// Returns true when `cache_file` records `fingerprint`.
pub fn is_fresh(cache_file: &Path, fingerprint: u64) -> bool {
    fs::read_to_string(cache_file)
        .ok()
        .and_then(|contents| contents.trim().parse::<u64>().ok())
        == Some(fingerprint)
}

/// Records `fingerprint` for the next build.
pub fn store(cache_file: &Path, fingerprint: u64) -> std::io::Result<()> {
    fs::write(cache_file, format!("{fingerprint}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_dir(name: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "tandem-cache-{}-{id}-{name}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn fingerprint_changes_with_sources() {
        let root = temp_dir("fingerprint");
        fs::write(root.join("main.tdm"), "int x = 1;").unwrap();
        let first = fingerprint(&root);
        fs::write(root.join("main.tdm"), "int x = 2;").unwrap();
        let second = fingerprint(&root);
        assert_ne!(first, second);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fingerprint_ignores_target_directory() {
        let root = temp_dir("ignored");
        fs::write(root.join("main.tdm"), "int x = 1;").unwrap();
        let first = fingerprint(&root);
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("target").join("generated.tdm"), "int y = 2;").unwrap();
        assert_eq!(first, fingerprint(&root));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cache_round_trips() {
        let root = temp_dir("round-trip");
        let cache = root.join(".fingerprint");
        assert!(!is_fresh(&cache, 42));
        store(&cache, 42).unwrap();
        assert!(is_fresh(&cache, 42));
        assert!(!is_fresh(&cache, 43));
        fs::remove_dir_all(root).unwrap();
    }
}
