use chronicle_storage::{AutomaticSnapshotOutcome, LocalRepository};
use std::{cell::Cell, fs};

#[test]
fn automatic_snapshot_deduplicates_only_valid_latest_and_preserves_manual_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("save.txt");
    fs::write(&source, "one").unwrap();
    let repo = LocalRepository::open(dir.path().join("repo")).unwrap();
    let entry = repo.add_entry(&source, None, None).unwrap();
    assert!(matches!(
        repo.create_automatic_snapshot(&entry.id, "auto", "test", &|| true)
            .unwrap(),
        AutomaticSnapshotOutcome::Created(_)
    ));
    assert!(matches!(
        repo.create_automatic_snapshot(&entry.id, "auto", "test", &|| true)
            .unwrap(),
        AutomaticSnapshotOutcome::Unchanged
    ));
    assert_eq!(repo.list_snapshots(&entry.id).unwrap().len(), 1);
    let latest = repo
        .create_snapshot(&entry.id, "manual", "test", false)
        .unwrap();
    assert_eq!(repo.list_snapshots(&entry.id).unwrap().len(), 2);
    fs::write(
        repo.entry_storage_path(&entry.id)
            .unwrap()
            .join(latest.archive_name),
        "broken",
    )
    .unwrap();
    assert!(matches!(
        repo.create_automatic_snapshot(&entry.id, "auto", "test", &|| true)
            .unwrap(),
        AutomaticSnapshotOutcome::Created(_)
    ));
    fs::write(&source, "two").unwrap();
    assert!(matches!(
        repo.create_automatic_snapshot(&entry.id, "auto", "test", &|| true)
            .unwrap(),
        AutomaticSnapshotOutcome::Created(_)
    ));
}

#[test]
fn automatic_snapshot_guard_prevents_commit_and_cleans_staging() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("save.txt");
    fs::write(&source, "one").unwrap();
    let repo = LocalRepository::open(dir.path().join("repo")).unwrap();
    let entry = repo.add_entry(&source, None, None).unwrap();
    let checks = Cell::new(0);
    let outcome = repo
        .create_automatic_snapshot(&entry.id, "auto", "test", &|| {
            checks.set(checks.get() + 1);
            checks.get() < 3
        })
        .unwrap();
    assert!(matches!(outcome, AutomaticSnapshotOutcome::Superseded));
    assert!(repo.list_snapshots(&entry.id).unwrap().is_empty());
    assert_eq!(fs::read_dir(repo.root().join(".tmp")).unwrap().count(), 0);
}
