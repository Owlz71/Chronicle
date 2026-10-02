#![allow(clippy::needless_pass_by_value)]

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::{AppState, storage_root};

/// Emitted while copying data so the settings dialog can show progress.
const MIGRATION_PROGRESS_EVENT: &str = "chronicle-storage-migration";
/// Progress is only reported once this many bytes were copied since the last event.
const PROGRESS_STEP_BYTES: u64 = 1024 * 1024;
/// Top-level repository directories that hold disposable data.
const TRANSIENT_DIRECTORIES: [&str; 1] = [".tmp"];

/// Which bootstrap pointer action to take once a migration succeeds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PointerAction {
    /// Point the bootstrap file at the new root.
    Write,
    /// Remove the bootstrap file so the default root is used again.
    Clear,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageLocationInfo {
    path: String,
    default_path: String,
    custom_path: Option<String>,
    portable: bool,
    unavailable: bool,
    total_bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationProgress {
    copied_bytes: u64,
    total_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMigrationResult {
    path: String,
    deleted_original: bool,
    warning: Option<String>,
    restart_required: bool,
}

#[tauri::command(async)]
pub fn storage_location_info(state: State<'_, AppState>) -> Result<StorageLocationInfo, String> {
    let layout = state.storage.clone();
    let repository = state
        .repository
        .lock()
        .map_err(|_| "Chronicle 本地仓库状态不可用".to_owned())?;
    let recycle = crate::commands::settings_recycle_root(&repository);
    let total_bytes = repository
        .total_stored_bytes_with_recycle(recycle.as_deref())
        .map_err(|error| error.to_string())?;
    Ok(StorageLocationInfo {
        path: layout.root.to_string_lossy().into_owned(),
        default_path: layout.default_root.to_string_lossy().into_owned(),
        custom_path: if layout.portable {
            None
        } else {
            storage_root::load_storage_location(&layout.location_file)
                .map(|path| path.to_string_lossy().into_owned())
        },
        portable: layout.portable,
        unavailable: layout.unavailable,
        total_bytes,
    })
}

#[tauri::command(async)]
pub fn migrate_storage_location(
    app: AppHandle,
    state: State<'_, AppState>,
    target_path: String,
) -> Result<StorageMigrationResult, String> {
    let target = PathBuf::from(target_path.trim());
    perform_migration(&app, state.inner(), target, PointerAction::Write)
}

/// Moves the repository back to the default location and clears the pointer.
#[tauri::command(async)]
pub fn reset_storage_location(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StorageMigrationResult, String> {
    let layout = state.storage.clone();
    if layout.portable {
        return Err("便携模式下不能更改存储位置".into());
    }
    if paths_equal(&layout.root, &layout.default_root) {
        storage_root::clear_storage_location(&layout.location_file)
            .map_err(|error| error.to_string())?;
        return Ok(StorageMigrationResult {
            path: layout.default_root.to_string_lossy().into_owned(),
            deleted_original: false,
            warning: None,
            restart_required: true,
        });
    }
    perform_migration(
        &app,
        state.inner(),
        layout.default_root.clone(),
        PointerAction::Clear,
    )
}

/// Restarts the application so the new repository root takes effect.
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}

fn perform_migration(
    app: &AppHandle,
    state: &AppState,
    target: PathBuf,
    pointer: PointerAction,
) -> Result<StorageMigrationResult, String> {
    let layout = state.storage.clone();
    if layout.portable {
        return Err("便携模式下不能更改存储位置".into());
    }
    let source = layout.root.clone();
    validate_migration_target(&source, &target)?;

    state.auto_backup.suspend()?;

    let total = match run_migration(app, &source, &target) {
        Ok(total) => total,
        Err(error) => {
            let _ = cleanup_partial_target(&target);
            let _ = state.auto_backup.resume();
            return Err(error);
        }
    };

    // Commit point: only after this write does the next launch boot from `target`.
    let pointer_result = match pointer {
        PointerAction::Write => storage_root::save_storage_location(&layout.location_file, &target),
        PointerAction::Clear => storage_root::clear_storage_location(&layout.location_file),
    };
    if let Err(error) = pointer_result {
        let _ = cleanup_partial_target(&target);
        let _ = state.auto_backup.resume();
        return Err(format!("无法保存存储位置配置：{error}"));
    }

    let _ = app.emit(
        MIGRATION_PROGRESS_EVENT,
        MigrationProgress {
            copied_bytes: total,
            total_bytes: total,
        },
    );

    let (deleted_original, warning) = match fs::remove_dir_all(&source) {
        Ok(()) => (true, None),
        Err(error) => (
            false,
            Some(format!("数据已迁移，但原数据目录删除失败：{error}")),
        ),
    };

    Ok(StorageMigrationResult {
        path: target.to_string_lossy().into_owned(),
        deleted_original,
        warning,
        restart_required: true,
    })
}

fn run_migration(app: &AppHandle, source: &Path, target: &Path) -> Result<u64, String> {
    let total = directory_size(source).map_err(|error| format!("无法统计原数据大小：{error}"))?;
    fs::create_dir_all(target).map_err(|error| format!("无法创建目标文件夹：{error}"))?;

    let mut copied = 0_u64;
    let mut last_emitted = 0_u64;
    copy_contents(source, target, true, &mut copied, &mut |current| {
        if current.saturating_sub(last_emitted) >= PROGRESS_STEP_BYTES || current == total {
            last_emitted = current;
            let _ = app.emit(
                MIGRATION_PROGRESS_EVENT,
                MigrationProgress {
                    copied_bytes: current,
                    total_bytes: total,
                },
            );
        }
    })
    .map_err(|error| format!("复制数据失败：{error}"))?;

    validate_migrated_repository(target)?;
    Ok(total)
}

fn validate_migration_target(current_root: &Path, target: &Path) -> Result<(), String> {
    if target.as_os_str().is_empty() {
        return Err("请选择新的存储位置".into());
    }
    if !target.is_absolute() {
        return Err("请选择绝对路径".into());
    }
    if target.parent().is_none() {
        return Err("请选择文件夹而不是磁盘根目录".into());
    }

    let current = normalize_path(current_root);
    let destination = normalize_path(target);
    if current == destination {
        return Err("新位置与当前位置相同".into());
    }
    if destination.starts_with(&current) {
        return Err("新位置不能位于当前数据目录内部".into());
    }
    if current.starts_with(&destination) {
        return Err("新位置不能包含当前数据目录".into());
    }

    if target.exists() {
        if !target.is_dir() {
            return Err("新位置必须是文件夹".into());
        }
        let mut entries =
            fs::read_dir(target).map_err(|error| format!("无法读取目标文件夹：{error}"))?;
        if entries.next().is_some() {
            return Err("请选择一个空文件夹".into());
        }
    } else {
        let parent = target
            .parent()
            .ok_or_else(|| "请选择有效的文件夹".to_owned())?;
        if !parent.exists() {
            return Err("目标文件夹的上级目录不存在".into());
        }
    }

    verify_writable(target)
}

fn verify_writable(target: &Path) -> Result<(), String> {
    let probe_directory = if target.is_dir() {
        target
    } else {
        target.parent().unwrap_or(target)
    };
    let probe = probe_directory.join(format!(".chronicle-probe-{}", uuid::Uuid::new_v4()));
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(error) => Err(format!("目标位置不可写：{error}")),
    }
}

fn validate_migrated_repository(target: &Path) -> Result<(), String> {
    if target.join("config/settings.json").is_file() || target.join("catalog.json").is_file() {
        Ok(())
    } else {
        Err("迁移后的数据缺少必要文件，已取消切换".into())
    }
}

/// Removes a partially copied target. Validation guarantees the target was
/// either created by us or empty beforehand, so no user data is at risk.
fn cleanup_partial_target(target: &Path) -> io::Result<()> {
    if target.is_dir() {
        fs::remove_dir_all(target)
    } else {
        Ok(())
    }
}

fn directory_size(root: &Path) -> io::Result<u64> {
    let mut total = 0_u64;
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| !is_transient_entry(root, entry.path()))
    {
        let entry = entry.map_err(|error| io::Error::other(error.to_string()))?;
        if entry.file_type().is_file() {
            let metadata = entry
                .metadata()
                .map_err(|error| io::Error::other(error.to_string()))?;
            total = total.saturating_add(metadata.len());
        }
    }
    Ok(total)
}

fn copy_contents(
    source: &Path,
    target: &Path,
    top_level: bool,
    copied: &mut u64,
    on_progress: &mut dyn FnMut(u64),
) -> io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        if top_level && is_transient_entry(source, &path) {
            continue;
        }
        let destination = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::create_dir_all(&destination)?;
            copy_contents(&path, &destination, false, copied, on_progress)?;
        } else if file_type.is_file() {
            fs::copy(&path, &destination)?;
            *copied = copied.saturating_add(entry.metadata()?.len());
            on_progress(*copied);
        }
    }
    Ok(())
}

fn is_transient_entry(root: &Path, path: &Path) -> bool {
    path != root
        && path.parent() == Some(root)
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| TRANSIENT_DIRECTORIES.contains(&name))
}

fn normalize_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => normalize_path(parent).join(name),
        _ => path.to_path_buf(),
    }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    normalize_path(left) == normalize_path(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "chronicle-migration-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn rejects_empty_and_relative_targets() {
        let root = temp_directory("rejects");
        assert!(validate_migration_target(&root, Path::new("")).is_err());
        assert!(validate_migration_target(&root, Path::new("relative/place")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_same_nested_and_overlapping_targets() {
        let root = temp_directory("overlap-root");
        let nested = root.join("inside");
        fs::create_dir_all(&nested).unwrap();

        assert!(validate_migration_target(&root, &root).is_err());
        assert!(validate_migration_target(&root, &nested).is_err());

        let outer = root.parent().unwrap().join(format!(
            "chronicle-migration-outer-{}",
            uuid::Uuid::new_v4()
        ));
        fs::remove_dir_all(&outer).ok();
        fs::create_dir_all(&outer).unwrap();
        assert!(validate_migration_target(&outer, &root).is_err());

        fs::remove_dir_all(&outer).ok();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_non_empty_target() {
        let root = temp_directory("non-empty-root");
        let target = temp_directory("non-empty-target");
        fs::write(target.join("keep.txt"), b"data").unwrap();

        assert!(validate_migration_target(&root, &target).is_err());

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(target).unwrap();
    }

    #[test]
    fn accepts_empty_target() {
        let root = temp_directory("empty-root");
        let target = temp_directory("empty-target");

        assert!(validate_migration_target(&root, &target).is_ok());

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(target).unwrap();
    }

    #[test]
    fn copies_repository_and_skips_transient_directories() {
        let source = temp_directory("copy-source");
        fs::create_dir_all(source.join("config")).unwrap();
        fs::create_dir_all(source.join("archives/entry-a")).unwrap();
        fs::create_dir_all(source.join(".tmp")).unwrap();
        fs::write(source.join("config/settings.json"), b"{}").unwrap();
        fs::write(source.join("catalog.json"), b"{}").unwrap();
        fs::write(source.join("archives/entry-a/snap.7z"), b"archive").unwrap();
        fs::write(source.join(".tmp/scratch.7z"), b"scratch").unwrap();

        let target = temp_directory("copy-target");
        fs::remove_dir_all(&target).unwrap();

        let mut copied = 0_u64;
        copy_contents(&source, &target, true, &mut copied, &mut |_| {}).unwrap();

        assert!(target.join("config/settings.json").is_file());
        assert!(target.join("catalog.json").is_file());
        assert!(target.join("archives/entry-a/snap.7z").is_file());
        assert!(!target.join(".tmp").exists());
        assert!(copied > 0);
        assert!(validate_migrated_repository(&target).is_ok());
        assert_eq!(directory_size(&source).unwrap(), copied);

        fs::remove_dir_all(source).unwrap();
        fs::remove_dir_all(target).unwrap();
    }

    #[test]
    fn reports_missing_repository_files() {
        let target = temp_directory("missing-files");
        assert!(validate_migrated_repository(&target).is_err());
        fs::remove_dir_all(target).unwrap();
    }
}
