use chronicle_core::StoragePolicy;
use chronicle_storage::{LocalRepository, health::*};
use std::{fs, sync::atomic::AtomicBool};

#[test]
fn health_detects_corruption_missing_and_pending_content_without_writing_sources() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("存档.txt");
    fs::write(&source, "one").unwrap();
    let repo = LocalRepository::open(temp.path().join("repo")).unwrap();
    let entry = repo
        .add_entry_sources(
            "test",
            &[source.to_string_lossy().into_owned()],
            None,
            StoragePolicy::Local,
        )
        .unwrap();
    let snapshot = repo
        .create_snapshot(&entry.id, "first", "test", false)
        .unwrap();
    let input = repo.health_inputs().unwrap().remove(0);
    let cancel = AtomicBool::new(false);
    let result = inspect_entry(&input, &cancel);
    assert_eq!(result.comparison, ContentComparison::Equal);
    assert_eq!(result.snapshots[0].code, HealthCode::Healthy);
    fs::write(&source, "two").unwrap();
    assert_eq!(
        inspect_entry(&input, &cancel).comparison,
        ContentComparison::Different
    );
    fs::write(&input.snapshots[0].path, "corrupt").unwrap();
    assert_eq!(
        inspect_entry(&input, &cancel).snapshots[0].code,
        HealthCode::HashMismatch
    );
    assert_eq!(
        inspect_entry(&input, &cancel).code,
        HealthCode::HashMismatch
    );
    fs::remove_file(&input.snapshots[0].path).unwrap();
    assert_eq!(
        inspect_entry(&input, &cancel).snapshots[0].code,
        HealthCode::Missing
    );
    assert_eq!(repo.list_snapshots(&entry.id).unwrap()[0].id, snapshot.id);
    assert_eq!(fs::read_to_string(source).unwrap(), "two");
}

#[test]
fn health_observation_uses_first_detection_not_snapshot_age() {
    let day = 86_400_000;
    let first = update_observation(None, &ContentComparison::Different, "a", 100, 7);
    assert!(!first.stale);
    assert!(
        !update_observation(
            first.observation.clone(),
            &ContentComparison::Different,
            "a",
            100 + 7 * day - 1,
            7
        )
        .stale
    );
    assert!(
        update_observation(
            first.observation.clone(),
            &ContentComparison::Different,
            "a",
            100 + 7 * day,
            7
        )
        .stale
    );
    assert!(
        !update_observation(
            first.observation.clone(),
            &ContentComparison::Unknown,
            "a",
            100 + 8 * day,
            7
        )
        .stale
    );
    assert!(
        !update_observation(
            first.observation.clone(),
            &ContentComparison::Different,
            "b",
            100 + 8 * day,
            7
        )
        .stale
    );
    assert!(
        !update_observation(
            first.observation.clone(),
            &ContentComparison::Different,
            "a",
            1,
            7
        )
        .stale
    );
    assert!(
        update_observation(first.observation, &ContentComparison::Equal, "a", 101, 7)
            .observation
            .is_none()
    );
}

#[test]
fn health_exclusions_unbound_missing_and_cancellation_are_not_false_success() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("游戏");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("save.dat"), "one").unwrap();
    let repo = LocalRepository::open(temp.path().join("repo")).unwrap();
    let entry = repo.add_entry(&folder, None, None).unwrap();
    repo.set_entry_exclusions(&entry.id, vec!["*.tmp".into()])
        .unwrap();
    repo.create_snapshot(&entry.id, "first", "test", false)
        .unwrap();
    fs::write(folder.join("cache.tmp"), "ignored").unwrap();
    let mut input = repo.health_inputs().unwrap().remove(0);
    assert_eq!(
        inspect_entry(&input, &AtomicBool::new(false)).comparison,
        ContentComparison::Equal
    );
    assert_eq!(
        inspect_entry(&input, &AtomicBool::new(true)).code,
        HealthCode::Cancelled
    );
    input.entry.sources[0].path.clear();
    assert_eq!(
        inspect_entry(&input, &AtomicBool::new(false)).sources[0].code,
        HealthCode::Unbound
    );
    input.entry.sources[0].path = temp.path().join("missing").display().to_string();
    assert_eq!(
        inspect_entry(&input, &AtomicBool::new(false)).comparison,
        ContentComparison::Unknown
    );
}
