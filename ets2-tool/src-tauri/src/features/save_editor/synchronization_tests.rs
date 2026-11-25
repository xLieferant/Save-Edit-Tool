use std::fs;
use std::path::PathBuf;

use super::synchronization::{inventory, publish_save, read_stable_save};
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

struct Fixture(PathBuf);

#[test]
fn repeated_same_size_same_timestamp_replacements_invalidate_both_caches() {
    let fixture = Fixture::new();
    let path = fixture.slot("profiles/test/save/quicksave");
    let cache = DecryptCache::default();
    let profiles = ProfileCache::default();
    let original = fs::read_to_string(&path).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    for money in [6500, 7600, 8700, 9800] {
        let text = super::document::patch_money(&original, money).unwrap();
        fs::write(&path, &text).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        assert_eq!(
            crate::shared::decrypt::decrypt_cached_with_cache(&path, &cache).unwrap(),
            text
        );
        assert!(
            profiles
                .get_save_game_data(&path.display().to_string())
                .is_none()
        );
        let snapshot = read_stable_save(&path).unwrap();
        assert_eq!(
            publish_save(&snapshot, &cache, &profiles).unwrap().money,
            Some(money)
        );
    }
}

#[test]
fn stable_save_writes_succeed_with_either_game_running() {
    let fixture = Fixture::new();
    for game in ["ets2", "ats"] {
        let path = fixture.slot(&format!("{game}/profiles/test/save/quicksave"));
        let path = fs::canonicalize(path).unwrap();
        let state = AppProfileState::default();
        *state.selected_game.lock().unwrap() = game.into();
        *state.current_profile.lock().unwrap() = Some(
            path.parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .display()
                .to_string(),
        );
        *state.current_save.lock().unwrap() = Some(path.display().to_string());
        super::safety::remember_save(&state, &path).unwrap();
        super::safety::with_test_game_running(true, || {
            let raw = super::safety::read_source(&state, &path).unwrap();
            let content = crate::shared::decrypt::decode_text_bytes(&raw, "test", &[]).unwrap();
            let updated = super::document::patch_money(&content, 76543).unwrap();
            super::safety::write_atomic_checked(&path, &raw, &updated, |candidate| {
                let written = fs::read_to_string(candidate).unwrap();
                let document = super::document::Document::parse(&written)?;
                assert_eq!(document.field(document.bank()?, "money_account")?, "76543");
                Ok(())
            })
            .unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), updated);
        });
    }
}

#[test]
fn external_change_after_validation_is_preserved() {
    let fixture = Fixture::new();
    let path = fixture.slot("profiles/test/save/quicksave");
    let original = fs::read_to_string(&path).unwrap();
    let updated = super::document::patch_money(&original, 12345).unwrap();
    let external = super::document::patch_money(&original, 54321).unwrap();
    let result = super::safety::write_atomic_checked(&path, original.as_bytes(), &updated, |_| {
        fs::write(&path, &external).unwrap();
        Ok(())
    });
    assert_eq!(result.unwrap_err(), "save_changed_since_load");
    assert_eq!(fs::read_to_string(&path).unwrap(), external);
}

#[test]
fn write_source_waits_for_a_partial_save_to_finish() {
    let fixture = Fixture::new();
    let path = fs::canonicalize(fixture.slot("profiles/test/save/quicksave")).unwrap();
    let complete = fs::read(&path).unwrap();
    fs::write(&path, "SiiNunit\n{\neconomy : pending {").unwrap();
    let state = AppProfileState::default();
    *state.current_profile.lock().unwrap() = Some(
        path.parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .display()
            .to_string(),
    );
    *state.current_save.lock().unwrap() = Some(path.display().to_string());
    let worker_path = path.clone();
    let expected = complete.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(180));
        fs::write(worker_path, complete).unwrap();
    });
    assert_eq!(super::safety::read_source(&state, &path).unwrap(), expected);
    worker.join().unwrap();
}

#[cfg(windows)]
#[test]
fn windows_temporary_lock_retries_but_permanent_lock_never_succeeds() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let path = fixture.slot("profiles/test/save/quicksave");
    let original = fs::read_to_string(&path).unwrap();
    let updated = super::document::patch_money(&original, 23456).unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(110));
        drop(lock);
    });
    super::safety::write_atomic_checked(&path, original.as_bytes(), &updated, |_| Ok(())).unwrap();
    release.join().unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    let result =
        super::safety::write_atomic_checked(&path, updated.as_bytes(), &original, |_| Ok(()));
    assert_eq!(result.unwrap_err(), "save_errors.locked");
    assert_eq!(fs::read_to_string(&path).unwrap(), updated);
    drop(lock);
    assert!(fs::read_dir(path.parent().unwrap()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
}

#[cfg(windows)]
#[test]
fn windows_retry_rechecks_original_after_lock_release() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let path = fixture.slot("profiles/test/save/quicksave");
    let original = fs::read_to_string(&path).unwrap();
    let updated = super::document::patch_money(&original, 34567).unwrap();
    let external = super::document::patch_money(&original, 76543).unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    let worker_path = path.clone();
    let worker_text = external.clone();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(80));
        drop(lock);
        fs::write(worker_path, worker_text).unwrap();
    });
    let result =
        super::safety::write_atomic_checked(&path, original.as_bytes(), &updated, |_| Ok(()));
    release.join().unwrap();
    assert_eq!(result.unwrap_err(), "save_changed_since_load");
    assert_eq!(fs::read_to_string(&path).unwrap(), external);
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("save-sync-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn slot(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("game.sii"), super::ats_tests::fixture()).unwrap();
        fs::write(path.join("info.sii"), "SiiNunit\n{\nsave_container : save.main {\n name: \"Live save\"\n info_visited_cities: 12\n}\n}\n").unwrap();
        path.join("game.sii")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn live_reads_for_both_games_do_not_require_process_exit_or_sdk() {
    let fixture = Fixture::new();
    for game in ["ats", "ets2"] {
        let path = fixture.slot(&format!("{game}/profiles/test/save/7"));
        let state = AppProfileState::default();
        *state.selected_game.lock().unwrap() = game.into();
        let cache = DecryptCache::default();
        let profile_cache = ProfileCache::default();
        super::safety::with_test_game_running(true, || {
            let save = read_stable_save(&path).unwrap();
            let first = publish_save(&save, &cache, &profile_cache).unwrap();
            assert_eq!(first.money, Some(6500));
            super::safety::remember_save(&state, &path).unwrap();
            let baseline = state.save_baseline.lock().unwrap().clone();
            let text = super::document::patch_money(&save.text, 9800).unwrap();
            fs::write(&path, text).unwrap();
            let changed = read_stable_save(&path).unwrap();
            assert_ne!(save.revision, changed.revision);
            assert_eq!(
                publish_save(&changed, &cache, &profile_cache)
                    .unwrap()
                    .money,
                Some(9800)
            );
            assert_eq!(
                *state.save_baseline.lock().unwrap(),
                baseline,
                "refresh must not accept the hash behind an open editor"
            );
            assert_eq!(
                super::safety::check_loaded_hash(&state, &path, &fs::read(&path).unwrap())
                    .unwrap_err(),
                "save_changed_since_load"
            );
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                changed.text,
                "refresh must never write a save"
            );
        });
    }
}

#[test]
fn corrupt_partial_read_keeps_last_successful_caches_and_hash() {
    let fixture = Fixture::new();
    let path = fixture.slot("profiles/test/save/7");
    let cache = DecryptCache::default();
    let profile_cache = ProfileCache::default();
    let save = read_stable_save(&path).unwrap();
    publish_save(&save, &cache, &profile_cache).unwrap();
    fs::write(&path, "SiiNunit\n{\neconomy : broken {").unwrap();
    assert!(read_stable_save(&path).is_err());
    assert_eq!(cache.files.lock().unwrap().get(&path), Some(&save.text));
    assert!(
        profile_cache
            .get_save_game_data(&path.display().to_string())
            .is_none(),
        "an externally changed file must not return stale profile data"
    );
}

#[test]
fn incomplete_metadata_retries_and_does_not_publish_partial_values() {
    let fixture = Fixture::new();
    let path = fixture.slot("profiles/test/save/autosave");
    fs::write(path.with_file_name("info.sii"), "SiiNunit\n{").unwrap();
    assert!(read_stable_save(&path).is_err());
}

#[test]
fn bounded_retries_recover_after_a_partial_write() {
    let fixture = Fixture::new();
    let path = fixture.slot("profiles/test/save/autosave");
    let original = fs::read(&path).unwrap();
    fs::write(&path, "SiiNunit\n{").unwrap();
    let worker_path = path.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        fs::write(worker_path, original).unwrap();
    });
    assert!(read_stable_save(&path).is_ok());
    worker.join().unwrap();
}

#[test]
fn metadata_change_is_a_new_revision_even_when_game_is_unchanged() {
    let fixture = Fixture::new();
    let path = fixture.slot("steam_profiles/test/save/7");
    let before = read_stable_save(&path).unwrap();
    let info = path.with_file_name("info.sii");
    fs::write(
        &info,
        fs::read_to_string(&info).unwrap().replace("12", "123"),
    )
    .unwrap();
    let after = read_stable_save(&path).unwrap();
    assert_eq!(before.hash, after.hash);
    assert_ne!(before.revision, after.revision);
}

#[test]
fn canonical_snapshot_invalidates_noncanonical_decrypt_cache_aliases() {
    let fixture = Fixture::new();
    let path = fixture.slot("profiles/test/save/7");
    let alias = path.parent().unwrap().join(".").join("game.sii");
    let canonical = fs::canonicalize(&path).unwrap();
    let cache = DecryptCache::default();
    let old = crate::shared::decrypt::decrypt_cached_with_cache(&alias, &cache).unwrap();
    fs::write(&path, super::document::patch_money(&old, 88000).unwrap()).unwrap();
    let updated = read_stable_save(&canonical).unwrap();
    publish_save(&updated, &cache, &ProfileCache::default()).unwrap();
    let reloaded = crate::shared::decrypt::decrypt_cached_with_cache(&alias, &cache).unwrap();
    assert_eq!(reloaded, updated.text);
    assert_ne!(reloaded, old);
}

#[test]
fn polling_detects_manual_autosave_overwrite_info_deletion_and_new_directories() {
    let fixture = Fixture::new();
    let roots = vec![fixture.0.join("profiles"), fixture.0.join("steam_profiles")];
    let initial = inventory(&roots, None).unwrap();
    let manual = fixture.slot("profiles/test/save/7");
    let first = inventory(&roots, None).unwrap();
    assert_ne!(initial, first);
    assert_eq!(
        first,
        inventory(&roots, None).unwrap(),
        "unchanged metadata must be a no-op"
    );
    fixture.slot("steam_profiles/test/save/autosave");
    let autosave = inventory(&roots, None).unwrap();
    assert_ne!(first, autosave);
    fs::write(&manual, format!("{}\n", super::ats_tests::fixture())).unwrap();
    let overwritten = inventory(&roots, None).unwrap();
    assert_ne!(autosave, overwritten);
    fs::write(manual.with_file_name("info.sii"), "partial").unwrap();
    let info = inventory(&roots, None).unwrap();
    assert_ne!(overwritten, info);
    fs::remove_dir_all(manual.parent().unwrap()).unwrap();
    let deleted = inventory(&roots, None).unwrap();
    assert_ne!(info, deleted);
    fs::create_dir_all(fixture.0.join("profiles/test/save/8")).unwrap();
    assert_ne!(deleted, inventory(&roots, None).unwrap());
}

#[test]
fn selected_custom_profile_is_watched_outside_default_roots() {
    let fixture = Fixture::new();
    let path = fixture.slot("custom-home/custom-profile/save/7");
    let profile = path.parent().unwrap().parent().unwrap().parent().unwrap();
    let roots = vec![fixture.0.join("missing-default")];
    assert!(inventory(&roots, None).unwrap().is_empty());
    assert!(
        inventory(&roots, Some(profile))
            .unwrap()
            .contains_key(&path)
    );
}

#[test]
fn native_watcher_and_polling_fallback_emit_debounced_events_and_stop() {
    use std::sync::mpsc;
    use std::time::Duration;
    use tauri::{Listener, Manager};
    for native in [true, false] {
        let fixture = Fixture::new();
        let path = fixture.slot("profiles/test/save/7");
        let app = tauri::test::mock_builder()
            .manage(AppProfileState::default())
            .manage(crate::state::SaveSyncState::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        *app.state::<AppProfileState>()
            .current_profile
            .lock()
            .unwrap() = Some(fixture.0.join("profiles/test").display().to_string());
        *app.state::<AppProfileState>().current_save.lock().unwrap() =
            Some(path.parent().unwrap().display().to_string());
        let (sender, receiver) = mpsc::channel();
        let listener = app.listen("savegame://changed", move |event| {
            let _ = sender.send(event.payload().to_string());
        });
        super::synchronization::start_with_roots(
            app.handle().clone(),
            Some(vec![fixture.0.join("profiles")]),
            native,
        );
        let initial = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(initial.contains("profiles"));
        for count in 1..=3 {
            fs::write(
                &path,
                format!("{}{}", super::ats_tests::fixture(), "\n".repeat(count)),
            )
            .unwrap();
            std::thread::sleep(Duration::from_millis(100));
        }
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            receiver.recv_timeout(Duration::from_secs(3)).is_err(),
            "unchanged files and burst events must not trigger repeated refreshes"
        );
        let metadata = fs::metadata(&path).unwrap();
        let inventory_before = inventory(&[fixture.0.join("profiles")], None).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        fs::write(&path, super::document::patch_money(&content, 7600).unwrap()).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(metadata.modified().unwrap())
            .unwrap();
        assert_eq!(
            inventory_before,
            inventory(&[fixture.0.join("profiles")], None).unwrap()
        );
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("polling must detect changed bytes even with identical size and timestamp");
        fixture.slot("profiles/test/save/autosave");
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        super::synchronization::stop(app.handle());
        fs::write(&path, "after stop").unwrap();
        assert!(receiver.recv_timeout(Duration::from_millis(250)).is_err());
        app.unlisten(listener);
    }
}

#[cfg(windows)]
#[test]
fn temporary_windows_sharing_violation_recovers_without_writing() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = Fixture::new();
    let path = fixture.slot("profiles/test/save/7");
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        drop(locked);
    });
    assert!(read_stable_save(&path).is_ok());
    worker.join().unwrap();
}

#[test]
fn synchronization_commands_preserve_conflicts_and_accept_only_the_current_revision() {
    let _commands = super::safety::TEST_COMMAND_LOCK.lock().unwrap();
    use serde_json::{Value, json};
    use tauri::{Manager, test::*};
    let fixture = Fixture::new();
    for game in ["ats", "ets2"] {
        let path = fixture.slot(&format!("{game}/profiles/live/save/7"));
        let profile = path.parent().unwrap().parent().unwrap().parent().unwrap();
        let slot = path.parent().unwrap();
        let app = mock_builder()
            .manage(AppProfileState::default())
            .manage(ProfileCache::default())
            .manage(DecryptCache::default())
            .manage(crate::state::SaveSyncState::default())
            .invoke_handler(tauri::generate_handler![
                crate::features::profile_manager::commands::set_selected_game,
                crate::features::profile_manager::commands::load_profile,
                super::synchronization::refresh_savegame,
                super::synchronization::accept_savegame_refresh,
            ])
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let call = |command: &str, body: Value| -> Result<Value, Value> {
            get_ipc_response(
                &window,
                tauri::webview::InvokeRequest {
                    cmd: command.to_string(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: "http://tauri.localhost".parse().unwrap(),
                    body: tauri::ipc::InvokeBody::Json(body),
                    headers: Default::default(),
                    invoke_key: INVOKE_KEY.to_string(),
                },
            )
            .map(|body| body.deserialize().unwrap())
        };
        super::safety::with_test_game_running(true, || {
            call("set_selected_game", json!({"game":game})).unwrap();
            call(
                "load_profile",
                json!({"profilePath":profile,"savePath":slot}),
            )
            .unwrap();
            let baseline = app
                .state::<AppProfileState>()
                .save_baseline
                .lock()
                .unwrap()
                .clone();
            let text = fs::read_to_string(&path).unwrap();
            fs::write(&path, super::document::patch_money(&text, 12345).unwrap()).unwrap();
            let context = json!({"game":game,"profilePath":profile,"savePath":slot,"force":true});
            let refreshed = call("refresh_savegame", context.clone()).unwrap();
            assert_eq!(refreshed["data"]["money"], 12345);
            assert_eq!(
                *app.state::<AppProfileState>().save_baseline.lock().unwrap(),
                baseline
            );
            let mut accept = context.clone();
            accept["revision"] = json!("stale");
            assert_eq!(
                call("accept_savegame_refresh", accept.clone()).unwrap_err(),
                "save_changed_during_read"
            );
            accept["revision"] = refreshed["revision"].clone();
            call("accept_savegame_refresh", accept).unwrap();
            assert_ne!(
                *app.state::<AppProfileState>().save_baseline.lock().unwrap(),
                baseline
            );
            let mut other = context.clone();
            other["game"] = json!(if game == "ats" { "ets2" } else { "ats" });
            assert_eq!(
                call("refresh_savegame", other).unwrap_err(),
                "save_selection_changed"
            );
            let sync = app.state::<crate::state::SaveSyncState>();
            let guard = sync.refresh_lock.lock().unwrap();
            assert_eq!(
                call("refresh_savegame", context.clone()).unwrap_err(),
                "save_refresh_in_progress"
            );
            drop(guard);
            fs::write(&path, "SiiNunit\n{").unwrap();
            assert!(call("refresh_savegame", context).is_err());
            let key = fs::canonicalize(&path).unwrap().display().to_string();
            assert!(
                app.state::<ProfileCache>()
                    .get_save_game_data(&key)
                    .is_none()
            );
        });
    }
}
