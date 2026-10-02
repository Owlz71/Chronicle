# Configurable Storage Location Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users move the entire local data repository to a folder of their choice, migrating data, deleting the old location, and restarting automatically.

**Architecture:** A bootstrap pointer file outside the repository records the custom root so the repository can be opened before its own settings load. The desktop app resolves portable mode, then the pointer, then the default. A new Rust module validates and performs the migration, suspends automatic backup during the copy, commits the pointer atomically, then deletes the source.

**Tech Stack:** Tauri 2 + Rust, chronicle-storage, Vue 3 + TypeScript, Vitest, Cargo tests.

**Spec:** `docs/superpowers/specs/2026-10-02-configurable-storage-location-design.md`

## Global Constraints

- The pointer file stays outside the repository; never store the custom root in `config/settings.json`.
- Commit order is copy → verify → write pointer → delete original; a failure before the commit must leave the running installation unchanged.
- Skip the transient `.tmp` directory when sizing, copying, and deleting.
- Portable mode (`portable.marker`) disables the feature.
- Keep the existing Chinese copy style; add English entries to `apps/desktop/src/locales/settings.ts`.
- No new Tauri plugins or capability permissions: restart uses the Rust `AppHandle::restart`.

---

### Task 1: Bootstrap pointer and layout resolution

**Files:**
- Modify: `apps/desktop/src-tauri/src/storage_root.rs`
- Modify: `apps/desktop/src-tauri/src/storage_root_tests.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

**Interfaces:**
- Produces `StorageLayout { root, location_file, default_root, portable, unavailable }` and `resolve_storage_layout(executable, app_local_data) -> io::Result<StorageLayout>`.
- Produces `load_storage_location`, `save_storage_location`, `clear_storage_location`, and `STORAGE_LOCATION_FILE`.

- [ ] **Step 1: Write failing resolver tests** for portable priority, custom override, invalid pointer, cleared pointer, and unavailable-root fallback.
- [ ] **Step 2: Run the resolver tests and verify they fail**

Run: `cargo test storage_root`

- [ ] **Step 3: Implement the pointer document, resolution order, usability check, and atomic writer.**
- [ ] **Step 4: Run the resolver tests and verify they pass**

Run: `cargo test storage_root`

### Task 2: Migration engine and commands

**Files:**
- Create: `apps/desktop/src-tauri/src/storage_migration.rs`
- Modify: `apps/desktop/src-tauri/src/auto_backup.rs`
- Modify: `apps/desktop/src-tauri/src/commands.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`

**Interfaces:**
- Produces `storage_location_info`, `migrate_storage_location(target_path)`, `reset_storage_location`, and `restart_app` commands.
- Produces `AutoBackupManager::suspend()` and `AutoBackupManager::resume()`.
- Consumes `commands::settings_recycle_root` (now `pub(crate)`).

- [ ] **Step 1: Write failing migration tests** covering target validation and a copy round-trip that skips `.tmp`.
- [ ] **Step 2: Run the migration tests and verify they fail**

Run: `cargo test storage_migration`

- [ ] **Step 3: Implement validation, recursive copy with progress events, verify, pointer commit, source deletion, and rollback.**
- [ ] **Step 4: Add `suspend`/`resume`, register the commands, and store the layout in `AppState`.**
- [ ] **Step 5: Run the migration tests and verify they pass**

Run: `cargo test storage_migration`

### Task 3: Frontend service layer

**Files:**
- Modify: `apps/desktop/src/domain.ts`
- Modify: `apps/desktop/src/services/repository.ts`
- Modify: `apps/desktop/src/services/archiveRepository.ts`

**Interfaces:**
- Produces `StorageLocationInfo` and `StorageMigrationResult` types and the matching repository methods on both adapters.

- [ ] **Step 1: Add the domain types and interface methods.**
- [ ] **Step 2: Implement the Tauri adapter methods and browser fallbacks.**
- [ ] **Step 3: Typecheck**

Run: `npm run typecheck`

### Task 4: Settings dialog

**Files:**
- Modify: `apps/desktop/src/components/SettingsDialog.vue`
- Modify: `apps/desktop/src/locales/settings.ts`

- [ ] **Step 1: Add storage state, load on open, and subscribe to the migration progress event.**
- [ ] **Step 2: Add the storage-location rows, confirmation dialog, progress, warning, and portable note.**
- [ ] **Step 3: Add English catalog entries for every new string.**
- [ ] **Step 4: Run the frontend tests and verify they pass**

Run: `npm test`

### Task 5: Verification

- [ ] **Step 1: Run the Rust tests**

Run: `cargo test`

- [ ] **Step 2: Manually change the location to an empty folder on another drive and confirm data is complete, the original folder is gone, and snapshots still restore.**
