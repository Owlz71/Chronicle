use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// File name of the bootstrap pointer that lives outside the repository.
pub(crate) const STORAGE_LOCATION_FILE: &str = "storage-location.json";
pub(crate) const STORAGE_LOCATION_VERSION: u32 = 1;

/// Resolved local storage layout, including the bootstrap pointer location.
///
/// The pointer file is deliberately kept outside the repository so a custom
/// repository root can be bootstrapped before the repository (and its
/// `config/settings.json`) has been opened.
#[derive(Clone, Debug)]
pub(crate) struct StorageLayout {
    /// The active repository root.
    pub root: PathBuf,
    /// Bootstrap pointer file (`<app local data>/storage-location.json`).
    pub location_file: PathBuf,
    /// Default root used when no custom path is configured.
    pub default_root: PathBuf,
    /// Whether the executable sits next to a `portable.marker`.
    pub portable: bool,
    /// Whether a configured custom root was temporarily unavailable.
    pub unavailable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StorageLocationDocument {
    pub format_version: u32,
    /// Absolute path to a user-selected repository root.
    pub repository_root: String,
}

/// Resolves the repository layout for the current launch.
///
/// Priority:
/// 1. `portable.marker` next to the executable → `<exe dir>/Chronicle-data`
/// 2. A valid `storage-location.json` pointer → its `repositoryRoot`
/// 3. Default `<app local data>/Chronicle`
///
/// # Errors
/// Returns an error when the executable has no parent directory.
pub(crate) fn resolve_storage_layout(
    executable: &Path,
    app_local_data: &Path,
) -> io::Result<StorageLayout> {
    let executable_directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("executable has no parent directory"))?;
    let location_file = app_local_data.join(STORAGE_LOCATION_FILE);
    let default_root = app_local_data.join("Chronicle");

    if executable_directory.join("portable.marker").is_file() {
        return Ok(StorageLayout {
            root: executable_directory.join("Chronicle-data"),
            location_file,
            default_root,
            portable: true,
            unavailable: false,
        });
    }

    let configured = load_storage_location(&location_file);
    let mut unavailable = false;
    let root = match configured {
        Some(custom) if storage_path_usable(&custom) => custom,
        Some(_) => {
            // Keep the pointer so the custom root is used again once it is back.
            unavailable = true;
            default_root.clone()
        }
        None => default_root.clone(),
    };
    Ok(StorageLayout {
        root,
        location_file,
        default_root,
        portable: false,
        unavailable,
    })
}

/// Whether a configured root can be used (or created) right now.
fn storage_path_usable(path: &Path) -> bool {
    if path.exists() {
        return path.is_dir();
    }
    path.parent().is_some_and(Path::is_dir)
}

/// Reads the custom repository root from the bootstrap pointer file.
///
/// Returns `None` when the file is missing, unreadable, or invalid so the
/// caller can fall back to the default location.
pub(crate) fn load_storage_location(location_file: &Path) -> Option<PathBuf> {
    let bytes = std::fs::read(location_file).ok()?;
    let document: StorageLocationDocument = serde_json::from_slice(&bytes).ok()?;
    if document.format_version != STORAGE_LOCATION_VERSION {
        return None;
    }
    let trimmed = document.repository_root.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

/// Atomically writes the bootstrap pointer file.
///
/// # Errors
/// Returns an error when the file has no parent directory or cannot be written.
pub(crate) fn save_storage_location(
    location_file: &Path,
    repository_root: &Path,
) -> io::Result<()> {
    let parent = location_file
        .parent()
        .ok_or_else(|| io::Error::other("storage location file has no parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let document = StorageLocationDocument {
        format_version: STORAGE_LOCATION_VERSION,
        repository_root: repository_root.to_string_lossy().into_owned(),
    };
    let bytes =
        serde_json::to_vec_pretty(&document).map_err(|error| io::Error::other(error.to_string()))?;
    let file_name = location_file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(STORAGE_LOCATION_FILE);
    let temporary = parent.join(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, location_file)?;
    Ok(())
}

/// Removes the bootstrap pointer so the app falls back to the default location.
///
/// # Errors
/// Returns an error when the file exists but cannot be removed.
pub(crate) fn clear_storage_location(location_file: &Path) -> io::Result<()> {
    match std::fs::remove_file(location_file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
