# Configurable Storage Location Design

## Goal

Let users move the whole local data repository (config, `7z` snapshots, catalog, cache) away from the default
`%LOCALAPPDATA%\com.thermalex.chronicle\Chronicle` folder to any drive, migrating existing data and restarting so the
change takes effect.

## Constraint

`config/settings.json` lives inside the repository, so the custom root cannot be stored in application settings without
a bootstrap cycle. A small pointer file therefore lives outside the repository.

- Pointer: `<app local data>/storage-location.json` (sibling of the default `Chronicle` folder).
- Document: `{ "formatVersion": 1, "repositoryRoot": "<absolute path>" }`.
- Residual size is a few hundred bytes; keeping it on `C:` is acceptable.

## Resolution Order

1. `portable.marker` next to the executable → `<exe dir>/Chronicle-data` (portable mode).
2. Valid `storage-location.json` whose root is usable → that root.
3. Default `<app local data>/Chronicle`.

A configured root is usable when it exists as a directory, or when its parent directory exists (so it can be created).
When a configured root is unusable (for example an unmounted drive), the app keeps the pointer, starts from the default
root, and reports the fallback so the UI can warn instead of failing to launch.

## Migration Flow

`migrate_storage_location(targetPath)`:

1. Validate: not portable; absolute path; not a drive root; different from and not nested with the current root; the
   target must not exist or must be an empty directory; the target must be writable.
2. Suspend automatic backup (clear watchers and invalidate in-flight workers).
3. Recursively copy the repository into the target, skipping the transient `.tmp` directory and emitting progress
   events (`chronicle-storage-migration`).
4. Verify the target contains `config/settings.json` or `catalog.json`.
5. Commit: atomically write the pointer file.
6. Delete the original directory (best effort; a failure is reported as a warning, not an error).
7. Return `{ path, deletedOriginal, warning, restartRequired }`; the frontend then calls `restart_app`.

Any failure before step 5 cleans up the partially copied target, rebuilds watchers, and leaves the current root
untouched, so the running installation stays consistent.

`reset_storage_location()` migrates back to the default root and removes the pointer.

## Interface

- Settings → Storage and backups gains a "Data storage location" row: the current path (click to open in the file
  explorer), a button to choose a new folder, and a "Restore default location" row when a custom root is active.
- Changing the location opens a confirmation dialog explaining that data is migrated, the original location deleted,
  and the app restarted. Progress and warnings are shown inline.
- Portable mode disables the control and explains why.

## Validation

- `storage_root` unit tests cover portable priority, custom root override, invalid pointers, cleared pointers, and the
  unavailable-root fallback.
- `storage_migration` unit tests cover target validation (empty/relative/overlapping/non-empty) and a copy round-trip
  that skips `.tmp` and verifies the required files.
- Existing frontend tests keep passing; new strings are added to the English catalog, and the placeholder test enforces
  consistent `{value1}` placeholders.
