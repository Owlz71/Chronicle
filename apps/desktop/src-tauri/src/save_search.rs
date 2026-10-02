#![allow(clippy::needless_pass_by_value)]
//! Finds candidate save folders under the current user's `AppData` for a dropped executable.
//!
//! Nothing is executed or modified; the search only reads directory names.

use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

/// Directory depth below each `AppData` root that is worth searching.
const MAX_DEPTH: usize = 5;
/// Upper bound on visited directories so a pathological tree cannot stall the UI.
const MAX_VISITED: usize = 40_000;
/// Shortest executable name worth fuzzy matching; anything shorter matches noise.
const MIN_FUZZY_LEN: usize = 3;
/// Upper bound on returned candidates.
const MAX_HITS: usize = 100;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSearchHit {
    path: String,
    name: String,
    exact: bool,
}

fn executable_stem(executable_path: &str) -> Option<String> {
    let stem = Path::new(executable_path.trim())
        .file_stem()?
        .to_str()?
        .trim()
        .to_owned();
    (!stem.is_empty()).then_some(stem)
}

/// The per-user application data roots, skipping duplicates and missing folders.
fn appdata_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for candidate in [dirs::config_dir(), dirs::data_local_dir()] {
        let Some(path) = candidate else {
            continue;
        };
        if path.is_dir() && !roots.contains(&path) {
            roots.push(path);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let local_low = home.join("AppData").join("LocalLow");
        if local_low.is_dir() && !roots.contains(&local_low) {
            roots.push(local_low);
        }
    }
    roots
}

/// Returns matching save folders with exact name matches first.
///
/// An exact match means a folder name equals the executable stem; a fuzzy match
/// means the stem appears inside the folder name. Results are bounded and the
/// search never fails: an unreadable directory is skipped instead of aborting.
#[tauri::command(async)]
pub fn search_appdata_saves(executable_path: String) -> Vec<SaveSearchHit> {
    let Some(stem) = executable_stem(&executable_path) else {
        return Vec::new();
    };
    let needle = stem.to_lowercase();
    let mut exact: Vec<SaveSearchHit> = Vec::new();
    let mut fuzzy: Vec<SaveSearchHit> = Vec::new();
    let mut visited = 0_usize;

    'roots: for root in appdata_roots() {
        let walker = WalkDir::new(&root)
            .min_depth(1)
            .max_depth(MAX_DEPTH)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| entry.file_type().is_dir());
        for entry in walker {
            if visited >= MAX_VISITED {
                break 'roots;
            }
            let Ok(entry) = entry else {
                continue;
            };
            visited += 1;
            let Some(name) = entry.file_name().to_str() else {
                continue;
            };
            let lowered = name.to_lowercase();
            let is_exact = lowered == needle;
            if !is_exact && (needle.len() < MIN_FUZZY_LEN || !lowered.contains(&needle)) {
                continue;
            }
            let hit = SaveSearchHit {
                path: entry.path().to_string_lossy().into_owned(),
                name: name.to_owned(),
                exact: is_exact,
            };
            if is_exact {
                exact.push(hit);
            } else {
                fuzzy.push(hit);
            }
        }
    }

    for group in [&mut exact, &mut fuzzy] {
        group.sort_by(|left, right| left.path.cmp(&right.path));
        group.dedup_by(|left, right| left.path == right.path);
    }
    exact.extend(fuzzy);
    exact.truncate(MAX_HITS);
    exact
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_executable_stem() {
        assert_eq!(
            executable_stem(r"C:\Games\My Game\My Game.exe").as_deref(),
            Some("My Game")
        );
        assert_eq!(
            executable_stem("  D:/tools/launcher.EXE  ").as_deref(),
            Some("launcher")
        );
        assert_eq!(executable_stem("   "), None);
    }

    #[test]
    fn appdata_roots_are_unique_and_existing() {
        let roots = appdata_roots();
        let mut unique = roots.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(roots.len(), unique.len());
        assert!(roots.iter().all(|root| root.is_dir()));
    }
}
