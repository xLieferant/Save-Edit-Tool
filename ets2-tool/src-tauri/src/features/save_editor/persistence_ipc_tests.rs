//! The production commands run through Tauri IPC against copied save slots.
//! The normal suite uses the existing repository ScsC fixture, not the user's
//! unavailable game(2).sii. The ignored test accepts that separate snapshot.
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tauri::test::{MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets};
use tauri::{Manager, WebviewWindow};

use super::coaches::{Career, patch_xp};

use super::document::{Document, patch_money, patch_skill};
use crate::features::backup::service::with_test_storage;

use crate::shared::decrypt::{decode_text_bytes, decrypt_if_needed, detect_signature};
use crate::state::{AppProfileState, DecryptCache, ProfileCache};

struct TestDirectory(PathBuf);
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn test_app() -> tauri::App<MockRuntime> {
    mock_builder()
        .manage(AppProfileState::default())
        .manage(ProfileCache::default())
        .manage(DecryptCache::default())
        .invoke_handler(tauri::generate_handler![
            crate::features::profile_manager::commands::set_selected_game,
            crate::features::profile_manager::commands::load_profile,
            crate::features::profile_manager::commands::set_current_save,
            crate::features::save_analysis::reader::read_all_save_data,
            crate::features::save_analysis::quicksave::quicksave_game_info,
            crate::features::save_editor::commands::read_project_coaches,
            crate::features::save_editor::commands::edit_player_money,
            crate::features::save_editor::commands::edit_career_xp,
            crate::features::save_editor::commands::edit_skill_value,
            crate::features::save_editor::commands::apply_custom_reset_values,
            crate::features::save_editor::commands::get_undo_status,
            crate::features::save_editor::commands::undo_last_save_change,
        ])
        .build(mock_context(noop_assets()))
        .unwrap()
}

fn call(window: &WebviewWindow<MockRuntime>, command: &str, body: Value) -> Result<Value, Value> {
    get_ipc_response(
        window,
        tauri::webview::InvokeRequest {
            cmd: command.to_string(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize().unwrap())
}

fn load(window: &WebviewWindow<MockRuntime>, game: &str, profile: &Path, save: &Path) {
    call(window, "set_selected_game", json!({"game":game})).unwrap();
    call(
        window,
        "load_profile",
        json!({"profilePath":profile,"savePath":save}),
    )
    .unwrap();
}

fn exercise_commands(game: &str, game_bytes: &[u8], info_bytes: &[u8]) {
    let root =
        TestDirectory(std::env::temp_dir().join(format!("save-ipc-{}", uuid::Uuid::new_v4())));
    let folder = if game == "ats" {
        "American Truck Simulator"
    } else {
        "Euro Truck Simulator 2"
    };
    let profile = root.0.join(folder).join("profiles/test");
    let selected = profile.join("save/7");
    let quicksave = profile.join("save/quicksave");
    let unrelated = root.0.join("unselected-profile/save/7");
    for directory in [&selected, &quicksave, &unrelated] {
        fs::create_dir_all(directory).unwrap();
        fs::write(directory.join("game.sii"), game_bytes).unwrap();
        fs::write(directory.join("info.sii"), info_bytes).unwrap();
    }
    let map = if game == "ats" {
        "/map/usa.mbd"
    } else {
        "/map/europe.mbd"
    };
    fs::write(
        profile.join("profile.sii"),
        format!("SiiNunit\n{{\nuser_profile : profile.test {{\n map_path: \"{map}\"\n}}\n}}\n"),
    )
    .unwrap();
    let game_path = selected.join("game.sii");
    let original = decode_text_bytes(game_bytes, "isolated_fixture", &[]).unwrap();
    let doc = Document::parse(&original).unwrap();
    let before_money = doc
        .field(doc.bank().unwrap(), "money_account")
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let before_xp = doc
        .field(doc.unique("economy").unwrap(), "experience_points")
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let before_skill = doc
        .field(doc.unique("economy").unwrap(), "long_dist")
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let money = if before_money == 1_234_567 {
        1_234_568
    } else {
        1_234_567
    };
    let xp = if before_xp == 50_000 { 50_001 } else { 50_000 };
    let skill = if before_skill == 5 { 4 } else { 5 };

    with_test_storage(&root.0.join("isolated-backups"), || {
        let app = test_app();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        load(&window, game, &profile, &selected);
        let loaded = call(&window, "read_all_save_data", json!({})).unwrap();
        assert_eq!(loaded["money"], before_money);
        assert_eq!(loaded["xp"], before_xp);

        #[cfg(windows)]
        let unlock = {
            use std::os::windows::fs::OpenOptionsExt;
            let lock = fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&game_path)
                .unwrap();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(180));
                drop(lock);
            })
        };
        super::safety::with_test_game_running(true, || {
            call(&window, "edit_player_money", json!({"value":money})).unwrap();
        });
        #[cfg(windows)]
        unlock.join().unwrap();
        let after_money = decrypt_if_needed(&game_path).unwrap();
        assert!(
            after_money == patch_money(&original, money).unwrap(),
            "only bank.money_account may change"
        );
        assert_eq!(
            call(&window, "read_all_save_data", json!({})).unwrap()["money"],
            money
        );

        let progression = call(&window, "read_project_coaches", json!({})).unwrap();
        call(
            &window,
            "edit_career_xp",
            json!({"xp":xp,"career":"truck","expectedSaveHash":progression["saveHash"]}),
        )
        .unwrap();
        let after_xp = decrypt_if_needed(&game_path).unwrap();
        assert!(
            after_xp == patch_xp(&after_money, Career::Truck, xp).unwrap(),
            "bus XP and other units must remain unchanged"
        );

        call(
            &window,
            "edit_skill_value",
            json!({"skill":"long_dist","value":skill}),
        )
        .unwrap();
        let after_skill = decrypt_if_needed(&game_path).unwrap();
        assert!(
            after_skill == patch_skill(&after_xp, "long_dist", skill).unwrap(),
            "unselected fields changed"
        );
        assert_eq!(
            call(&window, "quicksave_game_info", json!({})).unwrap()["long_dist"],
            skill
        );

        // Even a unique bank unit is not a substitute for a missing economy.bank link.
        let broken_slot = profile.join("save/8");
        fs::create_dir_all(&broken_slot).unwrap();
        let document = Document::parse(&after_skill).unwrap();
        let bank = document
            .field_span(document.unique("economy").unwrap(), "bank")
            .unwrap();
        let line_start = after_skill[..bank.start]
            .rfind('\n')
            .map(|position| position + 1)
            .unwrap_or(0);
        let line_end = after_skill[bank.end..]
            .find('\n')
            .map(|position| bank.end + position + 1)
            .unwrap_or(bank.end);
        let mut broken = after_skill.clone();
        broken.replace_range(line_start..line_end, "");
        fs::write(broken_slot.join("game.sii"), &broken).unwrap();
        fs::write(broken_slot.join("info.sii"), info_bytes).unwrap();
        load(&window, game, &profile, &broken_slot);
        assert!(call(&window, "edit_player_money", json!({"value":money + 1})).is_err());
        assert!(
            fs::read(broken_slot.join("game.sii")).unwrap() == broken.as_bytes(),
            "missing reference must never select a fallback bank"
        );
        load(&window, game, &profile, &selected);

        // Invalid values and a save outside the active profile cannot succeed.
        assert!(call(&window, "edit_player_money", json!({"value":-1})).is_err());
        assert!(
            call(
                &window,
                "load_profile",
                json!({"profilePath":profile,"savePath":unrelated})
            )
            .is_err()
        );
        assert!(
            decrypt_if_needed(&game_path).unwrap() == after_skill,
            "rejected operation changed the selected save"
        );
        assert!(
            fs::read(quicksave.join("game.sii")).unwrap() == game_bytes,
            "quicksave changed"
        );
        assert!(
            fs::read(unrelated.join("game.sii")).unwrap() == game_bytes,
            "unrelated profile changed"
        );
        assert!(
            fs::read(selected.join("info.sii")).unwrap() == info_bytes,
            "ordinary profile edits preserve metadata"
        );

        let database =
            rusqlite::Connection::open(root.0.join("isolated-backups/backups.sqlite")).unwrap();
        let count: i64 = database
            .query_row("SELECT COUNT(*) FROM ets_save_backups", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(
            count, 3,
            "each successful mutation creates a persistent backup"
        );
        let (backups, storage): (String, String) = database.query_row(
            "SELECT files_json, storage_dir FROM ets_save_backups ORDER BY created_at_utc LIMIT 1", [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        let files: Vec<crate::features::backup::models::BackupFileRecord> =
            serde_json::from_str(&backups).unwrap();
        let canonical_game = fs::canonicalize(&game_path).unwrap();
        let game_backup = files
            .iter()
            .find(|file| {
                fs::canonicalize(&file.live_path).ok().as_deref() == Some(canonical_game.as_path())
            })
            .expect("selected game backup missing");
        let stored_bytes = fs::read(Path::new(&storage).join(&game_backup.stored_path)).unwrap();
        assert!(
            stored_bytes == game_bytes,
            "backup must preserve the original encrypted bytes"
        );
        assert_eq!(
            game_backup.checksum,
            crate::shared::ets2data::validate::sha256_hex_bytes(game_bytes)
        );
        drop(database);
        drop(window);
        drop(app);

        // Fresh app state, empty caches and the normal load command must agree.
        let restarted = test_app();
        let window = tauri::WebviewWindowBuilder::new(&restarted, "main", Default::default())
            .build()
            .unwrap();
        load(&window, game, &profile, &selected);
        let loaded = call(&window, "read_all_save_data", json!({})).unwrap();
        assert_eq!(loaded["money"], money);
        assert_eq!(loaded["xp"], xp);
        assert_eq!(
            call(&window, "quicksave_game_info", json!({})).unwrap()["long_dist"],
            skill
        );
        let current = restarted.state::<AppProfileState>();
        assert_eq!(*current.selected_game.lock().unwrap(), game);
        assert_eq!(
            current.current_save.lock().unwrap().as_deref(),
            selected.to_str()
        );
    });
}

#[test]
fn shared_scs_c_pipeline_persists_through_real_tauri_commands_for_both_games() {
    let _commands = super::safety::TEST_COMMAND_LOCK.lock().unwrap();
    let game = include_bytes!("../../../test-fixtures/decrypt/encrypted_game.sii");
    assert_eq!(detect_signature(game).signature, "aes_text");
    // This metadata deliberately disagrees with the existing repository fixture.
    let info = b"SiiNunit\n{\nsave_container : save.test {\n version: 105\n name: \"test\"\n info_money_account: 2847\n info_players_experience: 41\n}\n}\n";
    super::safety::with_test_game_running(false, || {
        for selected_game in ["ets2", "ats"] {
            exercise_commands(selected_game, game, info);
        }
    });
}

#[test]
fn repeated_live_quicksave_overwrites_and_custom_reset_undo_preserve_original_bytes() {
    let _commands = super::safety::TEST_COMMAND_LOCK.lock().unwrap();
    let root = TestDirectory(
        std::env::temp_dir().join(format!("live-quicksave-ipc-{}", uuid::Uuid::new_v4())),
    );
    for game in ["ets2", "ats"] {
        let profile = root.0.join(game).join("profiles/live");
        let slot = profile.join("save/quicksave");
        fs::create_dir_all(&slot).unwrap();
        let path = slot.join("game.sii");
        let source = super::ats_tests::fixture();
        let info = "SiiNunit\n{\nsave_container : save.main {\n name: \"Quicksave\"\n info_money_account: 6500\n}\n}\n";
        fs::write(&path, &source).unwrap();
        fs::write(slot.join("info.sii"), info).unwrap();
        with_test_storage(&root.0.join(format!("backups-{game}")), || {
            super::safety::with_test_game_running(true, || {
                let app = test_app();
                let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
                    .build()
                    .unwrap();
                load(&window, game, &profile, &slot);
                assert_eq!(
                    call(&window, "read_all_save_data", json!({})).unwrap()["money"],
                    6500
                );
                let modified = fs::metadata(&path).unwrap().modified().unwrap();
                for money in [7600, 8700, 9800] {
                    let external = patch_money(&source, money).unwrap();
                    fs::write(&path, &external).unwrap();
                    fs::OpenOptions::new()
                        .write(true)
                        .open(&path)
                        .unwrap()
                        .set_modified(modified)
                        .unwrap();
                    assert_eq!(
                        call(&window, "read_all_save_data", json!({})).unwrap()["money"],
                        money
                    );
                    assert_eq!(
                        call(&window, "edit_player_money", json!({"value":12345})).unwrap_err(),
                        "save_changed_since_load"
                    );
                    assert_eq!(fs::read_to_string(&path).unwrap(), external);
                    call(&window, "set_current_save", json!({"savePath":slot})).unwrap();
                    call(&window, "apply_custom_reset_values", json!({"money":12345})).unwrap();
                    assert_eq!(
                        call(&window, "read_all_save_data", json!({})).unwrap()["money"],
                        12345
                    );
                    let metadata = decrypt_if_needed(&slot.join("info.sii")).unwrap();
                    let document = Document::parse(&metadata).unwrap();
                    assert_eq!(
                        document
                            .field(
                                document.unique("save_container").unwrap(),
                                "info_money_account"
                            )
                            .unwrap(),
                        "12345"
                    );
                    assert_eq!(
                        call(&window, "get_undo_status", json!({})).unwrap()["canUndo"],
                        true
                    );
                    call(&window, "undo_last_save_change", json!({})).unwrap();
                    assert_eq!(fs::read(&path).unwrap(), external.as_bytes());
                    assert_eq!(fs::read(slot.join("info.sii")).unwrap(), info.as_bytes());
                    assert_eq!(
                        app.state::<AppProfileState>()
                            .current_save
                            .lock()
                            .unwrap()
                            .as_deref(),
                        slot.to_str()
                    );
                }
            });
        });
    }
}

#[test]
#[ignore = "Requires the separate new snapshot: ATS_GAME_REFERENCE=game(2).sii and ATS_INFO_REFERENCE=info(2).sii"]
fn provided_ats_snapshot_persists_through_tauri_and_restart() {
    let game =
        fs::read(std::env::var_os("ATS_GAME_REFERENCE").expect("set ATS_GAME_REFERENCE")).unwrap();
    let info =
        fs::read(std::env::var_os("ATS_INFO_REFERENCE").expect("set ATS_INFO_REFERENCE")).unwrap();
    assert_eq!(&game[..4], b"ScsC");
    assert!(info.starts_with(b"SiiNunit"));
    super::safety::with_test_game_running(false, || exercise_commands("ats", &game, &info));
}
