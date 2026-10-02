use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::storage_root::{
    STORAGE_LOCATION_FILE, clear_storage_location, load_storage_location, resolve_storage_layout,
    save_storage_location,
};

fn test_directory(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("chronicle-{name}-{}", uuid::Uuid::new_v4()))
}

#[test]
fn portable_marker_uses_sibling_data_directory() {
    let directory = test_directory("portable-root");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("portable.marker"), "").unwrap();

    let layout =
        resolve_storage_layout(&directory.join("Chronicle.exe"), Path::new("C:/AppData")).unwrap();

    assert_eq!(layout.root, directory.join("Chronicle-data"));
    assert!(layout.portable);
    assert_eq!(layout.default_root, PathBuf::from("C:/AppData").join("Chronicle"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn normal_installation_uses_app_local_data() {
    let directory = test_directory("installed-root");

    let layout =
        resolve_storage_layout(&directory.join("Chronicle.exe"), Path::new("C:/AppData")).unwrap();

    assert_eq!(layout.root, PathBuf::from("C:/AppData").join("Chronicle"));
    assert!(!layout.portable);
    assert_eq!(
        layout.location_file,
        PathBuf::from("C:/AppData").join(STORAGE_LOCATION_FILE)
    );
}

#[test]
fn custom_location_overrides_default() {
    let app_local_data = test_directory("custom-location");
    fs::create_dir_all(&app_local_data).unwrap();
    let target = test_directory("custom-target");
    save_storage_location(&app_local_data.join(STORAGE_LOCATION_FILE), &target).unwrap();

    let layout =
        resolve_storage_layout(&app_local_data.join("Chronicle.exe"), &app_local_data).unwrap();

    assert_eq!(layout.root, target);
    assert!(!layout.portable);
    assert_eq!(
        load_storage_location(&app_local_data.join(STORAGE_LOCATION_FILE)),
        Some(target)
    );
    fs::remove_dir_all(app_local_data).unwrap();
}

#[test]
fn portable_marker_takes_priority_over_custom_location() {
    let directory = test_directory("portable-priority");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("portable.marker"), "").unwrap();
    let app_local_data = directory.join("AppData");
    let custom = test_directory("portable-custom-target");
    save_storage_location(&app_local_data.join(STORAGE_LOCATION_FILE), &custom).unwrap();

    let layout =
        resolve_storage_layout(&directory.join("Chronicle.exe"), &app_local_data).unwrap();

    assert_eq!(layout.root, directory.join("Chronicle-data"));
    assert!(layout.portable);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn invalid_location_file_falls_back_to_default() {
    let app_local_data = test_directory("invalid-location");
    fs::create_dir_all(&app_local_data).unwrap();
    fs::write(app_local_data.join(STORAGE_LOCATION_FILE), "{not json").unwrap();

    let layout =
        resolve_storage_layout(&app_local_data.join("Chronicle.exe"), &app_local_data).unwrap();

    assert_eq!(layout.root, app_local_data.join("Chronicle"));
    assert_eq!(load_storage_location(&app_local_data.join(STORAGE_LOCATION_FILE)), None);
    fs::remove_dir_all(app_local_data).unwrap();
}

#[test]
fn unavailable_custom_location_falls_back_to_default() {
    let app_local_data = test_directory("unavailable-location");
    fs::create_dir_all(&app_local_data).unwrap();
    let pointer = app_local_data.join(STORAGE_LOCATION_FILE);
    // Parent directory does not exist, so the configured root cannot be used.
    let missing = test_directory("unavailable-target").join("missing").join("deep");
    save_storage_location(&pointer, &missing).unwrap();

    let layout =
        resolve_storage_layout(&app_local_data.join("Chronicle.exe"), &app_local_data).unwrap();

    assert_eq!(layout.root, app_local_data.join("Chronicle"));
    assert!(layout.unavailable);
    assert_eq!(load_storage_location(&pointer), Some(missing));
    fs::remove_dir_all(app_local_data).unwrap();
}

#[test]
fn cleared_location_falls_back_to_default() {
    let app_local_data = test_directory("cleared-location");
    fs::create_dir_all(&app_local_data).unwrap();
    let pointer = app_local_data.join(STORAGE_LOCATION_FILE);
    save_storage_location(&pointer, &test_directory("cleared-target")).unwrap();
    clear_storage_location(&pointer).unwrap();

    let layout =
        resolve_storage_layout(&app_local_data.join("Chronicle.exe"), &app_local_data).unwrap();

    assert_eq!(layout.root, app_local_data.join("Chronicle"));
    fs::remove_dir_all(app_local_data).unwrap();
}
