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
/// Upper bound on returned candidates.
const MAX_HITS: usize = 100;

/// Words that carry no search value, so `Game_launcher_x64.exe` still finds `Game`.
const NOISE_TOKENS: &[&str] = &[
    "app",
    "application",
    "bin",
    "boot",
    "bootstrapper",
    "client",
    "crash",
    "exe",
    "install",
    "installer",
    "launch",
    "launcher",
    "patch",
    "player",
    "redist",
    "report",
    "setup",
    "start",
    "startup",
    "steam",
    "unins",
    "uninstall",
    "update",
    "updater",
    "win",
    "win32",
    "win64",
    "x64",
    "x86",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSearchHit {
    pub path: String,
    pub name: String,
    pub exact: bool,
    /// `folder` or `file`; some engines keep saves as loose files next to the game.
    pub kind: String,
}

impl SaveSearchHit {
    pub(crate) fn folder(path: String, name: String, exact: bool) -> Self {
        Self {
            path,
            name,
            exact,
            kind: "folder".to_owned(),
        }
    }

    pub(crate) fn file(path: String, name: String) -> Self {
        Self {
            path,
            name,
            exact: false,
            kind: "file".to_owned(),
        }
    }
}

struct Candidate {
    path: String,
    name: String,
    tier: u8,
    matched: usize,
}

impl Candidate {
    fn to_hit(&self) -> SaveSearchHit {
        SaveSearchHit::folder(self.path.clone(), self.name.clone(), self.tier == 0)
    }
}

fn executable_stem(executable_path: &str) -> Option<String> {
    let stem = Path::new(executable_path.trim())
        .file_stem()?
        .to_str()?
        .trim()
        .to_owned();
    (!stem.is_empty()).then_some(stem)
}

/// Replaces every separator with a space and drops apostrophes, keeping the case.
fn spaced(value: &str) -> String {
    let mut spaced = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '\'' | '\u{2019}' | '\u{02bc}' | '`') {
            continue;
        }
        spaced.push(if character.is_alphanumeric() {
            character
        } else {
            ' '
        });
    }
    spaced
        .split(' ')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Case-insensitive, separator-insensitive form used for exact comparisons.
fn normalize(value: &str) -> String {
    spaced(value).to_lowercase()
}

/// Splits `SlayTheSpire2` into `Slay`, `The`, `Spire2` so spaced folder names match.
fn split_camel_case(word: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for character in word.chars() {
        if previous_lower && character.is_uppercase() && !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        }
        previous_lower = character.is_lowercase();
        current.push(character);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Drops tokens that carry no search value: single ASCII letters, bare digits,
/// version numbers and build-tool suffixes such as `launcher` or `x64`.
fn is_noise(token: &str) -> bool {
    if token.is_ascii() && token.chars().count() < 2 {
        return true;
    }
    if token.chars().all(|character| character.is_ascii_digit()) {
        return true;
    }
    if let Some(digits) = token.strip_prefix('v')
        && !digits.is_empty()
        && digits.chars().all(|character| character.is_ascii_digit())
    {
        return true;
    }
    NOISE_TOKENS.contains(&token)
}

/// Splits the executable name into the keywords a user would type in a search box.
fn query_tokens(stem: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    for word in spaced(stem).split(' ') {
        for part in split_camel_case(word) {
            let lowered = part.to_lowercase();
            if !is_noise(&lowered) {
                tokens.push(lowered);
            }
        }
    }
    tokens.sort();
    tokens.dedup();
    if tokens.is_empty() {
        let fallback = normalize(stem);
        if !fallback.is_empty() {
            tokens.push(fallback);
        }
    }
    tokens
}

/// Ranks a folder name against the query. Tier 0 is an exact name, tier 1 matches
/// every keyword and tier 2 matches most keywords in any order.
fn classify(name: &str, normalized: &str, tokens: &[String]) -> Option<(u8, usize)> {
    let candidate = normalize(name);
    if candidate.is_empty() {
        return None;
    }
    if candidate == normalized {
        return Some((0, tokens.len()));
    }
    let matched = tokens
        .iter()
        .filter(|token| candidate.contains(token.as_str()))
        .count();
    if matched == 0 {
        return None;
    }
    if matched == tokens.len() {
        return Some((1, matched));
    }
    // A partial overlap only counts once at least two keywords hit, so a single
    // shared word such as `demon` cannot flood the list with unrelated folders.
    if matched >= 2 && matched * 2 >= tokens.len() {
        return Some((2, matched));
    }
    None
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

/// Searches `AppData` for folders matching the dropped executable by keyword, the
/// way a search engine does: `Maxwell's_demon.exe` also finds `maxwells_puzzling_demon`.
///
/// Exact name matches come first, then folders containing every keyword, then
/// folders containing most keywords. Results are bounded and the search never
/// fails: an unreadable directory is skipped instead of aborting.
#[tauri::command(async)]
pub fn search_appdata_saves(executable_path: String) -> Vec<SaveSearchHit> {
    let Some(stem) = executable_stem(&executable_path) else {
        return Vec::new();
    };
    let normalized = normalize(&stem);
    if normalized.is_empty() {
        return Vec::new();
    }
    let tokens = query_tokens(&stem);
    let mut candidates: Vec<Candidate> = Vec::new();
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
            let Some((tier, matched)) = classify(name, &normalized, &tokens) else {
                continue;
            };
            candidates.push(Candidate {
                path: entry.path().to_string_lossy().into_owned(),
                name: name.to_owned(),
                tier,
                matched,
            });
        }
    }

    candidates.sort_by(|left, right| {
        left.tier
            .cmp(&right.tier)
            .then_with(|| right.matched.cmp(&left.matched))
            .then_with(|| left.name.len().cmp(&right.name.len()))
            .then_with(|| left.path.cmp(&right.path))
    });
    candidates.truncate(MAX_HITS);
    candidates.iter().map(Candidate::to_hit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify_name(name: &str, executable: &str) -> Option<(u8, usize)> {
        classify(name, &normalize(executable), &query_tokens(executable))
    }

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
    fn drops_apostrophes_instead_of_splitting_the_word() {
        assert_eq!(normalize("Maxwell's_demon"), "maxwells demon");
        assert_eq!(query_tokens("Maxwell's_demon"), ["demon", "maxwells"]);
    }

    #[test]
    fn splits_camel_case_into_keywords() {
        assert_eq!(query_tokens("SlayTheSpire2"), ["slay", "spire2", "the"]);
        assert_eq!(
            classify_name("Slay the Spire 2", "SlayTheSpire2"),
            Some((2, 2))
        );
        assert_eq!(
            classify_name("SlayTheSpire2", "SlayTheSpire2"),
            Some((0, 3))
        );
    }

    #[test]
    fn matches_keywords_in_any_order_and_position() {
        assert_eq!(
            classify_name("maxwells_puzzling_demon", "Maxwell's_demon"),
            Some((1, 2))
        );
        assert_eq!(classify_name("exact name", "exact name"), Some((0, 2)));
    }

    #[test]
    fn ignores_a_single_shared_keyword() {
        assert_eq!(classify_name("demon_tools", "Maxwell's_demon"), None);
        assert_eq!(classify_name("something_else", "Maxwell's_demon"), None);
    }

    #[test]
    fn drops_launcher_and_version_noise() {
        assert_eq!(query_tokens("Game_launcher_v1.2_win64"), ["game"]);
        assert_eq!(query_tokens("x64"), ["x64"]);
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
