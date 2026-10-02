#![allow(clippy::needless_pass_by_value)]
//! Identifies what a dropped folder or executable is, and where that game keeps saves.
//!
//! Only the first level of the dropped folder is inspected, plus a few fixed probes such
//! as `www/js` and `Data`. Nothing is ever executed; only directory names and small text
//! files are read.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::save_search::{SaveSearchHit, search_appdata_saves};

/// Executables shipped alongside a game that are never the game itself.
const HELPER_EXE_PREFIXES: &[&str] = &[
    "crashhandler",
    "dotnetfx",
    "dxsetup",
    "dxwebsetup",
    "oalinst",
    "unins",
    "unitycrashhandler",
    "vcredist",
];

/// Exact executable names that only ever belong to installers.
const HELPER_EXE_NAMES: &[&str] = &["install", "setup"];

/// Saves written next to the game by the older RPG Maker engines.
const LOOSE_SAVE_EXTENSIONS: &[&str] =
    &["lsd", "rmmzsave", "rpgsave", "rvdata", "rvdata2", "rxdata"];

/// Directory names that hold saves inside a game folder.
const SAVE_DIRECTORY_NAMES: &[&str] = &["save", "savedata", "saves"];

/// Company or product names that Unity fills in when nothing was configured.
const PLACEHOLDER_NAMES: &[&str] = &[
    "companyname",
    "default company",
    "defaultcompany",
    "productname",
    "unknown",
    "unity",
];

const MAX_EXE_CANDIDATES: usize = 40;
const MAX_SAVE_HITS: usize = 60;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameEngine {
    Unity,
    RpgMaker2000,
    RpgMakerXp,
    RpgMakerVx,
    RpgMakerVxAce,
    RpgMakerMv,
    RpgMakerMz,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExeCandidate {
    pub path: String,
    pub name: String,
    /// The pairing with `<stem>_Data` identifies the real Unity player.
    pub likely: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedGameInspection {
    pub engine: GameEngine,
    pub root: String,
    pub exe_candidates: Vec<ExeCandidate>,
    pub recommended_exe: Option<String>,
}

fn child_directories(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

/// Case-insensitive lookup of one direct child.
fn find_child(root: &Path, wanted: &str) -> Option<PathBuf> {
    let wanted = wanted.to_lowercase();
    child_directories_and_files(root).into_iter().find(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.to_lowercase() == wanted)
    })
}

fn child_directories_and_files(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries.flatten().map(|entry| entry.path()).collect()
}

/// `<name>_Data` containing `globalgamemanagers` only exists in a built player, never in
/// an editor project folder, so it is the reliable marker for a shipped Unity game.
fn unity_data_directory(root: &Path) -> Option<PathBuf> {
    child_directories(root).into_iter().find(|directory| {
        directory
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.to_lowercase().ends_with("_data"))
            && directory.join("globalgamemanagers").is_file()
    })
}

/// Extracts the player name from `<name>_Data`.
fn unity_player_stem(directory: &Path) -> Option<String> {
    let name = directory.file_name()?.to_str()?;
    let stem_length = name.len().checked_sub("_Data".len())?;
    if stem_length == 0 {
        return None;
    }
    let (stem, suffix) = name.split_at(stem_length);
    suffix
        .eq_ignore_ascii_case("_Data")
        .then(|| stem.to_owned())
}

/// `js/<file>` or `www/js/<file>`, the two layouts RPG Maker MV and MZ ship with.
fn has_engine_script(root: &Path, file: &str) -> bool {
    root.join("js").join(file).is_file() || root.join("www").join("js").join(file).is_file()
}

fn is_nwjs_game(root: &Path) -> bool {
    root.join("nw.dll").is_file()
        || (root.join("package.json").is_file()
            && (has_engine_script(root, "rpg_core.js") || has_engine_script(root, "rmmz_core.js")))
}

/// The `Data` folder extension identifies the RGSS generation.
fn rgss_extension(root: &Path) -> Option<&'static str> {
    let directory = find_child(root, "Data")?;
    let entries = fs::read_dir(&directory).ok()?;
    for entry in entries.flatten().take(400) {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let lowered = name.to_lowercase();
        for extension in ["rvdata2", "rvdata", "rxdata"] {
            if lowered.ends_with(&format!(".{extension}")) {
                return Some(extension);
            }
        }
    }
    None
}

fn detect_engine(root: &Path) -> GameEngine {
    if root.join("UnityPlayer.dll").is_file() || unity_data_directory(root).is_some() {
        return GameEngine::Unity;
    }
    if find_child(root, "RPG_RT.ldb").is_some() || find_child(root, "RPG_RT.exe").is_some() {
        return GameEngine::RpgMaker2000;
    }
    if is_nwjs_game(root) {
        return if has_engine_script(root, "rmmz_core.js") {
            GameEngine::RpgMakerMz
        } else {
            GameEngine::RpgMakerMv
        };
    }
    match rgss_extension(root) {
        Some("rxdata") => GameEngine::RpgMakerXp,
        Some("rvdata") => GameEngine::RpgMakerVx,
        Some("rvdata2") => GameEngine::RpgMakerVxAce,
        _ => GameEngine::Unknown,
    }
}

fn is_helper_executable(stem: &str) -> bool {
    let lowered = stem.to_lowercase();
    HELPER_EXE_NAMES.contains(&lowered.as_str())
        || HELPER_EXE_PREFIXES
            .iter()
            .any(|prefix| lowered.starts_with(prefix))
}

/// Executables directly inside `root` with the helper programs removed.
fn executable_candidates(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = child_directories_and_files(root)
        .into_iter()
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        })
        .filter(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| !is_helper_executable(stem))
        })
        .collect();
    paths.sort();
    paths.truncate(MAX_EXE_CANDIDATES);
    paths
}

/// Resolves the folder to inspect and the executables worth offering.
#[tauri::command(async)]
pub fn inspect_dropped_game(path: String) -> DroppedGameInspection {
    let dropped = PathBuf::from(path.trim());
    let root = if dropped.is_dir() {
        dropped.clone()
    } else {
        dropped
            .parent()
            .map_or_else(|| dropped.clone(), Path::to_path_buf)
    };
    let engine = if root.is_dir() {
        detect_engine(&root)
    } else {
        GameEngine::Unknown
    };
    let expected_player = unity_data_directory(&root)
        .as_deref()
        .and_then(unity_player_stem);
    let mut exe_candidates: Vec<ExeCandidate> = executable_candidates(&root)
        .into_iter()
        .map(|candidate| {
            let name = candidate
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned();
            let likely = expected_player.as_ref().is_some_and(|expected| {
                candidate
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem.eq_ignore_ascii_case(expected))
            });
            ExeCandidate {
                path: candidate.to_string_lossy().into_owned(),
                name,
                likely,
            }
        })
        .collect();
    exe_candidates.sort_by(|left, right| {
        right
            .likely
            .cmp(&left.likely)
            .then_with(|| left.name.cmp(&right.name))
    });
    let recommended_exe = if exe_candidates.len() == 1 {
        exe_candidates
            .first()
            .map(|candidate| candidate.path.clone())
    } else {
        exe_candidates
            .iter()
            .find(|candidate| candidate.likely)
            .map(|candidate| candidate.path.clone())
    };
    DroppedGameInspection {
        engine,
        root: root.to_string_lossy().into_owned(),
        exe_candidates,
        recommended_exe,
    }
}

/// Unity keeps user data beside the roaming profile, in priority order.
fn unity_appdata_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("AppData").join("LocalLow"));
    }
    roots.extend(
        [dirs::data_local_dir(), dirs::config_dir()]
            .into_iter()
            .flatten(),
    );
    roots.retain(|root| root.is_dir());
    roots
}

fn is_placeholder(value: &str) -> bool {
    PLACEHOLDER_NAMES.contains(&value.to_lowercase().as_str())
}

/// `app.info` holds the company name and then the product name on its first two lines.
fn read_unity_app_info(root: &Path) -> Option<(String, String)> {
    let directory = unity_data_directory(root)?;
    let raw = fs::read_to_string(directory.join("app.info")).ok()?;
    let mut lines = raw.lines().map(str::trim).filter(|line| !line.is_empty());
    let company = lines.next()?.to_owned();
    let product = lines.next()?.to_owned();
    if is_placeholder(&company) || is_placeholder(&product) {
        return None;
    }
    Some((company, product))
}

fn push_folder(hits: &mut Vec<SaveSearchHit>, path: PathBuf, exact: bool) {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return;
    };
    hits.push(SaveSearchHit::folder(
        path.to_string_lossy().into_owned(),
        name.to_owned(),
        exact,
    ));
}

/// `LocalLow\<company>\<product>` is the documented Unity location; the keyword search is
/// appended as a safety net because the names do not always have to match the game name.
fn collect_unity_saves(root: &Path, executable_path: &str, hits: &mut Vec<SaveSearchHit>) {
    if let Some((company, product)) = read_unity_app_info(root) {
        for base in unity_appdata_roots() {
            let candidate = base.join(&company).join(&product);
            if candidate.is_dir() {
                push_folder(hits, candidate, true);
            }
        }
    }
    hits.extend(search_appdata_saves(executable_path.to_owned()));
}

/// Only ever yields a `save` style folder, never a game root: a root would back up the
/// whole installation instead of the player's progress.
fn collect_directory_saves(root: &Path, include_www: bool, hits: &mut Vec<SaveSearchHit>) {
    let mut bases = vec![root.to_path_buf()];
    if include_www {
        bases.extend(child_directories(root).into_iter().filter(|directory| {
            directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("www"))
        }));
    }
    for base in bases {
        for directory in child_directories(&base) {
            let Some(name) = directory.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let lowered = name.to_lowercase();
            if SAVE_DIRECTORY_NAMES.contains(&lowered.as_str()) {
                push_folder(hits, directory, true);
            }
        }
    }
}

/// RPG Maker XP, VX, VX Ace and 2000 write loose save files into the game folder, so the
/// files themselves become the sources.
fn collect_loose_saves(root: &Path, hits: &mut Vec<SaveSearchHit>) {
    let mut files: Vec<PathBuf> = child_directories_and_files(root)
        .into_iter()
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    LOOSE_SAVE_EXTENSIONS
                        .iter()
                        .any(|known| extension.eq_ignore_ascii_case(known))
                })
        })
        .collect();
    files.sort();
    for file in files {
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        hits.push(SaveSearchHit::file(
            file.to_string_lossy().into_owned(),
            name,
        ));
    }
}

/// NW.js stores web storage in `<userData>\<product>\Local Storage`, and that folder name
/// is not guaranteed to match the product name, so only an exact stem match is accepted.
fn collect_local_storage(executable_path: &str, hits: &mut Vec<SaveSearchHit>) {
    let Some(stem) = Path::new(executable_path.trim())
        .file_stem()
        .and_then(|stem| stem.to_str())
    else {
        return;
    };
    for base in [dirs::config_dir(), dirs::data_local_dir()]
        .into_iter()
        .flatten()
    {
        for directory in child_directories(&base) {
            let matches_stem = directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(stem));
            if !matches_stem {
                continue;
            }
            let storage = directory.join("Local Storage");
            if storage.is_dir() {
                push_folder(hits, storage, false);
            }
        }
    }
}

fn dedupe(hits: &mut Vec<SaveSearchHit>) {
    let mut seen = BTreeSet::new();
    hits.retain(|hit| seen.insert(hit.path.to_lowercase()));
    hits.truncate(MAX_SAVE_HITS);
}

/// Looks for the save locations of a game the user just dropped.
///
/// The engine decides where to look; when nothing turns up, the keyword search over
/// `AppData` is used as the safety net.
#[tauri::command(async)]
pub fn search_game_saves(
    root: String,
    executable_path: String,
    engine: GameEngine,
) -> Vec<SaveSearchHit> {
    let root_path = PathBuf::from(root.trim());
    let mut hits: Vec<SaveSearchHit> = Vec::new();
    match engine {
        GameEngine::Unity => collect_unity_saves(&root_path, &executable_path, &mut hits),
        GameEngine::RpgMakerMv | GameEngine::RpgMakerMz => {
            collect_directory_saves(&root_path, true, &mut hits);
            collect_local_storage(&executable_path, &mut hits);
        }
        GameEngine::RpgMaker2000
        | GameEngine::RpgMakerXp
        | GameEngine::RpgMakerVx
        | GameEngine::RpgMakerVxAce => collect_loose_saves(&root_path, &mut hits),
        GameEngine::Unknown => {}
    }
    if hits.is_empty() {
        hits = search_appdata_saves(executable_path);
    }
    dedupe(&mut hits);
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("chronicle-launch-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn touch(path: PathBuf) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "x").unwrap();
    }

    fn unity_player(root: &Path) {
        touch(root.join("UnityPlayer.dll"));
        touch(root.join("Game_Data").join("globalgamemanagers"));
        touch(root.join("Game.exe"));
        touch(root.join("UnityCrashHandler64.exe"));
    }

    #[test]
    fn unity_player_keeps_only_the_real_game_executable() {
        let root = workspace("unity");
        unity_player(&root);
        let report = inspect_dropped_game(root.to_string_lossy().into_owned());
        assert_eq!(report.engine, GameEngine::Unity);
        let names: Vec<_> = report
            .exe_candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect();
        assert_eq!(names, ["Game.exe"]);
        assert!(report.exe_candidates[0].likely);
        assert_eq!(
            report.recommended_exe.as_deref(),
            Some(root.join("Game.exe").to_string_lossy().as_ref())
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_unity_editor_project_is_not_a_built_player() {
        let root = workspace("project");
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::create_dir_all(root.join("ProjectSettings")).unwrap();
        assert_eq!(detect_engine(&root), GameEngine::Unknown);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rpg_maker_engines_are_told_apart_by_their_data_folder() {
        for (extension, engine) in [
            ("rxdata", GameEngine::RpgMakerXp),
            ("rvdata", GameEngine::RpgMakerVx),
            ("rvdata2", GameEngine::RpgMakerVxAce),
        ] {
            let root = workspace(&format!("rgss-{extension}"));
            touch(root.join("Game.exe"));
            touch(root.join("Data").join(format!("System.{extension}")));
            assert_eq!(detect_engine(&root), engine);
            fs::remove_dir_all(&root).unwrap();
        }
        let nwjs = workspace("nwjs");
        touch(nwjs.join("Game.exe"));
        touch(nwjs.join("nw.dll"));
        touch(nwjs.join("www").join("js").join("rmmz_core.js"));
        assert_eq!(detect_engine(&nwjs), GameEngine::RpgMakerMz);
        fs::remove_dir_all(&nwjs).unwrap();
    }

    #[test]
    fn rm_save_search_never_offers_the_game_root() {
        let root = workspace("rm-root");
        touch(root.join("Game.exe"));
        touch(root.join("Data").join("System.rvdata2"));
        touch(root.join("Save1.rvdata2"));
        let hits = search_game_saves(
            root.to_string_lossy().into_owned(),
            root.join("Game.exe").to_string_lossy().into_owned(),
            GameEngine::RpgMakerVxAce,
        );
        assert!(hits.iter().all(|hit| hit.path != root.to_string_lossy()));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Save1.rvdata2");
        assert_eq!(hits[0].kind, "file");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn mv_save_search_returns_the_save_directory_not_the_www_folder() {
        let root = workspace("mv-root");
        touch(root.join("Game.exe"));
        touch(root.join("nw.dll"));
        touch(root.join("www").join("js").join("rpg_core.js"));
        touch(root.join("www").join("save").join("file1.rpgsave"));
        let hits = search_game_saves(
            root.to_string_lossy().into_owned(),
            root.join("Game.exe").to_string_lossy().into_owned(),
            GameEngine::RpgMakerMv,
        );
        assert_eq!(hits[0].name, "save");
        assert_eq!(hits[0].kind, "folder");
        assert!(hits[0].path.ends_with("www\\save") || hits[0].path.ends_with("www/save"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn helper_executables_are_recognised_by_prefix_and_name() {
        assert!(is_helper_executable("UnityCrashHandler64"));
        assert!(is_helper_executable("unins000"));
        assert!(is_helper_executable("setup"));
        assert!(!is_helper_executable("Game"));
        assert!(!is_helper_executable("Ever17"));
    }
}
