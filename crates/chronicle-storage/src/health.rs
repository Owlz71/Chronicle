//! Read-only, cancellable local backup inspection. A hash check is not a restore test.
use crate::exclusions::ExclusionRules;
use chronicle_core::{Entry, EntryKind, Snapshot};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug)]
pub struct HealthSnapshotInput {
    pub snapshot: Snapshot,
    pub path: PathBuf,
}
#[derive(Clone, Debug)]
pub struct HealthEntryInput {
    pub entry: Entry,
    pub snapshots: Vec<HealthSnapshotInput>,
    pub fingerprint: String,
}
impl HealthEntryInput {
    /// # Errors
    /// Returns an error when source or snapshot metadata cannot be serialized.
    pub fn new(
        entry: Entry,
        mut snapshots: Vec<HealthSnapshotInput>,
    ) -> Result<Self, serde_json::Error> {
        snapshots.sort_by_key(|s| std::cmp::Reverse(s.snapshot.created_at_ms));
        let identities: Vec<_> = snapshots
            .iter()
            .map(|s| {
                (
                    &s.snapshot.id,
                    &s.snapshot.object_hash,
                    &s.path,
                    &s.snapshot.files,
                )
            })
            .collect();
        let bytes = serde_json::to_vec(&(&entry.sources, &entry.exclude_patterns, identities))?;
        let fingerprint =
            Sha256::digest(bytes)
                .iter()
                .fold(String::with_capacity(64), |mut text, byte| {
                    let _ = write!(text, "{byte:02x}");
                    text
                });
        Ok(Self {
            entry,
            snapshots,
            fingerprint,
        })
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthCode {
    Healthy,
    Missing,
    PermissionDenied,
    Unavailable,
    Unbound,
    HashMismatch,
    ReadError,
    Skipped,
    ChangedDuringCheck,
    NoSnapshots,
    PendingChanges,
    StaleChanges,
    Cancelled,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContentComparison {
    Equal,
    Different,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthItem {
    pub id: String,
    pub path: String,
    pub code: HealthCode,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthEntryResult {
    pub entry_id: String,
    pub name: String,
    pub sources: Vec<HealthItem>,
    pub snapshots: Vec<HealthItem>,
    pub comparison: ContentComparison,
    pub code: HealthCode,
    pub last_snapshot_at: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingObservation {
    pub baseline: String,
    pub first_observed_at: u64,
    pub last_observed_at: u64,
}
pub struct ObservationUpdate {
    pub observation: Option<PendingObservation>,
    pub stale: bool,
}
#[must_use]
pub fn update_observation(
    previous: Option<PendingObservation>,
    comparison: &ContentComparison,
    baseline: &str,
    now_ms: u64,
    stale_days: u16,
) -> ObservationUpdate {
    if *comparison == ContentComparison::Unknown {
        return ObservationUpdate {
            observation: previous,
            stale: false,
        };
    }
    if *comparison == ContentComparison::Equal {
        return ObservationUpdate {
            observation: None,
            stale: false,
        };
    }
    let mut observation = previous
        .filter(|p| p.baseline == baseline && p.last_observed_at <= now_ms)
        .unwrap_or(PendingObservation {
            baseline: baseline.into(),
            first_observed_at: now_ms,
            last_observed_at: now_ms,
        });
    observation.last_observed_at = now_ms;
    let stale = now_ms.saturating_sub(observation.first_observed_at)
        >= u64::from(stale_days.clamp(1, 365)) * 86_400_000;
    ObservationUpdate {
        observation: Some(observation),
        stale,
    }
}

type ProbeResult<T> = Result<T, (HealthCode, String)>;
// Kept by value to use directly with Result::map_err.
#[allow(clippy::needless_pass_by_value)]
fn io_error(error: io::Error) -> (HealthCode, String) {
    let code = match error.kind() {
        io::ErrorKind::NotFound => HealthCode::Missing,
        io::ErrorKind::PermissionDenied => HealthCode::PermissionDenied,
        _ => HealthCode::ReadError,
    };
    (code, error.to_string())
}
fn cancelled(flag: &AtomicBool) -> ProbeResult<()> {
    if flag.load(Ordering::Relaxed) {
        Err((HealthCode::Cancelled, String::new()))
    } else {
        Ok(())
    }
}
/// Inspect every ancestor before opening anything, including placeholders and junctions.
/// # Errors
/// Returns a diagnostic for unbound, inaccessible, linked or placeholder paths.
pub fn safe_metadata(path: &Path) -> ProbeResult<fs::Metadata> {
    if !path.is_absolute() {
        return Err((HealthCode::Unbound, String::new()));
    }
    let mut result = None;
    for ancestor in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
        let metadata = fs::symlink_metadata(ancestor).map_err(io_error)?;
        let unsafe_path = metadata.file_type().is_symlink();
        #[cfg(windows)]
        let unsafe_path = {
            use std::os::windows::fs::MetadataExt;
            unsafe_path
                || metadata.file_attributes() & (0x400 | 0x1000 | 0x40000 | 0x0040_0000) != 0
        };
        if unsafe_path {
            return Err((HealthCode::Skipped, ancestor.display().to_string()));
        }
        result = Some(metadata);
    }
    result.ok_or((HealthCode::Unbound, String::new()))
}
/// # Errors
/// Returns a diagnostic when cancelled, unreadable, unsafe or changed during hashing.
pub fn hash_checked(path: &Path, flag: &AtomicBool) -> ProbeResult<String> {
    cancelled(flag)?;
    let before = safe_metadata(path)?;
    if !before.is_file() {
        return Err((HealthCode::ReadError, "not_a_file".into()));
    }
    let mut file = fs::File::open(path).map_err(io_error)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; 256 * 1024];
    loop {
        cancelled(flag)?;
        let len = file.read(&mut buffer).map_err(io_error)?;
        if len == 0 {
            break;
        }
        hash.update(&buffer[..len]);
    }
    let after = safe_metadata(path)?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return Err((HealthCode::ChangedDuringCheck, String::new()));
    }
    Ok(hash
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        }))
}
fn probe_registry(path: &str) -> ProbeResult<()> {
    let normalized =
        crate::registry::normalize_path(path).map_err(|e| (HealthCode::ReadError, e))?;
    #[cfg(windows)]
    {
        use winreg::{
            RegKey,
            enums::{
                HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, REG_LINK,
                REG_OPTION_OPEN_LINK,
            },
        };
        let (hive, sub) = normalized
            .split_once('\\')
            .ok_or((HealthCode::ReadError, String::new()))?;
        let mut key = RegKey::predef(if hive == "HKEY_CURRENT_USER" {
            HKEY_CURRENT_USER
        } else {
            HKEY_LOCAL_MACHINE
        });
        for part in sub.split('\\') {
            key = key
                .open_subkey_with_options_flags(
                    part,
                    REG_OPTION_OPEN_LINK,
                    KEY_READ | KEY_WOW64_64KEY,
                )
                .map_err(io_error)?;
            for value in key.enum_values() {
                if value.map_err(io_error)?.1.vtype == REG_LINK {
                    return Err((HealthCode::Skipped, "registry_link".into()));
                }
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = normalized;
        Err((HealthCode::Skipped, "windows_only".into()))
    }
}
pub(crate) fn source_manifest(
    source: &chronicle_core::EntrySource,
    rules: &ExclusionRules,
    flag: &AtomicBool,
) -> ProbeResult<BTreeMap<String, String>> {
    if source.path.is_empty() {
        return Err((HealthCode::Unbound, String::new()));
    }
    if source.kind == EntryKind::Registry {
        return read_source_manifest(source, rules, flag);
    }
    let before = source_stamps(source, rules, flag)?;
    let manifest = read_source_manifest(source, rules, flag)?;
    if before != source_stamps(source, rules, flag)? {
        return Err((HealthCode::ChangedDuringCheck, String::new()));
    }
    Ok(manifest)
}

// Recheck the whole tree, not only each open file: additions and writes to a file
// already hashed must not produce a confident comparison of a moving target.
fn source_stamps(
    source: &chronicle_core::EntrySource,
    rules: &ExclusionRules,
    flag: &AtomicBool,
) -> ProbeResult<BTreeMap<PathBuf, (u64, Option<std::time::SystemTime>)>> {
    let mut stamps = BTreeMap::new();
    for (path, meta) in source_files(source, rules, flag)? {
        stamps.insert(path, (meta.len(), meta.modified().ok()));
    }
    Ok(stamps)
}

fn source_files(
    source: &chronicle_core::EntrySource,
    rules: &ExclusionRules,
    flag: &AtomicBool,
) -> ProbeResult<Vec<(PathBuf, fs::Metadata)>> {
    let root = Path::new(&source.path);
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(path) = pending.pop() {
        cancelled(flag)?;
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        let relative = if path == root && source.kind == EntryKind::File {
            Path::new(&source.name)
        } else {
            path.strip_prefix(root).unwrap_or(&path)
        };
        if rules.is_excluded(relative, metadata.is_dir()) {
            continue;
        }
        let metadata = safe_metadata(&path)?;
        if metadata.is_dir() {
            // Unlike WalkDir, validate attributes before opening the directory.
            for child in fs::read_dir(&path).map_err(io_error)? {
                pending.push(child.map_err(io_error)?.path());
            }
        } else if metadata.is_file() {
            files.push((path, metadata));
        }
    }
    Ok(files)
}

fn read_source_manifest(
    source: &chronicle_core::EntrySource,
    rules: &ExclusionRules,
    flag: &AtomicBool,
) -> ProbeResult<BTreeMap<String, String>> {
    cancelled(flag)?;
    if source.path.is_empty() {
        return Err((HealthCode::Unbound, String::new()));
    }
    if source.kind == EntryKind::Registry {
        probe_registry(&source.path)?;
        return Ok(BTreeMap::new());
    }
    let root = Path::new(&source.path);
    let metadata = safe_metadata(root)?;
    let mut result = BTreeMap::new();
    if source.kind == EntryKind::File {
        if !rules.is_excluded(Path::new(&source.name), false) {
            result.insert(
                format!("{}/{}", source.id, source.name),
                hash_checked(root, flag)?,
            );
        }
    } else {
        if !metadata.is_dir() {
            return Err((HealthCode::ReadError, "not_a_directory".into()));
        }
        for (path, _) in source_files(source, rules, flag)? {
            let relative = path
                .strip_prefix(root)
                .map_err(|e| (HealthCode::ReadError, e.to_string()))?;
            result.insert(
                format!(
                    "{}/{}",
                    source.id,
                    relative.to_string_lossy().replace('\\', "/")
                ),
                hash_checked(&path, flag)?,
            );
        }
    }
    Ok(result)
}
// Keep the ordered source/snapshot checks and final result classification together.
#[allow(clippy::too_many_lines)]
pub fn inspect_entry(input: &HealthEntryInput, flag: &AtomicBool) -> HealthEntryResult {
    let mut result = HealthEntryResult {
        entry_id: input.entry.id.clone(),
        name: input.entry.name.clone(),
        sources: vec![],
        snapshots: vec![],
        comparison: ContentComparison::Unknown,
        code: HealthCode::Healthy,
        last_snapshot_at: input.snapshots.first().map(|s| s.snapshot.created_at_ms),
    };
    let Ok(rules) = ExclusionRules::new(&input.entry.exclude_patterns) else {
        result.code = HealthCode::ReadError;
        return result;
    };
    let mut manifest = BTreeMap::new();
    let mut complete = !input.entry.sources.is_empty();
    for source in &input.entry.sources {
        let (code, detail) = match source_manifest(source, &rules, flag) {
            Ok(files) => {
                manifest.extend(files);
                (HealthCode::Healthy, String::new())
            }
            Err(e) => {
                complete = false;
                e
            }
        };
        result.sources.push(HealthItem {
            id: source.id.clone(),
            path: source.path.clone(),
            code,
            detail,
        });
    }
    if input.entry.sources.is_empty() {
        result.code = HealthCode::Unbound;
    }
    for item in &input.snapshots {
        let (code, detail) = match hash_checked(&item.path, flag) {
            Ok(hash) => (
                if hash == item.snapshot.object_hash {
                    HealthCode::Healthy
                } else {
                    HealthCode::HashMismatch
                },
                String::new(),
            ),
            Err(e) => e,
        };
        result.snapshots.push(HealthItem {
            id: item.snapshot.id.clone(),
            path: item.path.display().to_string(),
            code,
            detail,
        });
        if flag.load(Ordering::Relaxed) {
            break;
        }
    }
    if input.snapshots.is_empty() {
        result.code = HealthCode::NoSnapshots;
    }
    let file_sources: Vec<_> = input
        .entry
        .sources
        .iter()
        .filter(|s| s.kind != EntryKind::Registry)
        .collect();
    if complete
        && !file_sources.is_empty()
        && let Some(latest) = input.snapshots.first()
    {
        let previous: BTreeMap<String, String> = latest
            .snapshot
            .files
            .iter()
            .filter(|f| {
                file_sources.iter().any(|s| {
                    f.relative_path
                        .strip_prefix(&format!("{}/", s.id))
                        .is_some_and(|p| !rules.is_excluded(Path::new(p), false))
                })
            })
            .map(|f| (f.relative_path.clone(), f.content_hash.clone()))
            .collect();
        result.comparison = if manifest == previous {
            ContentComparison::Equal
        } else {
            ContentComparison::Different
        };
        if result.comparison == ContentComparison::Different {
            result.code = HealthCode::PendingChanges;
        }
    }
    if let Some(issue) = result
        .sources
        .iter()
        .chain(&result.snapshots)
        .find(|item| item.code != HealthCode::Healthy)
    {
        result.code = issue.code;
    }
    if flag.load(Ordering::Relaxed) {
        result.code = HealthCode::Cancelled;
        result.comparison = ContentComparison::Unknown;
    }
    result
}
