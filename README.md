# ETS2 Save Edit Tool

### Speicherformat und lokale Profile
Bei der Spielauswahl und beim manuellen Refresh prüft das Tool die globale `config.cfg` für ETS2 beziehungsweise ATS auf `uset g_save_format "2"`. Änderungen und das Ergänzen eines fehlenden Eintrags erfolgen ausschließlich nach Bestätigung. Vorher wird eine unveränderte Sicherung als `config.cfg.save-format-<UUID>.bak` neben der Originaldatei erstellt. Fehlende oder nicht lesbare Config-Dateien werden als Status angezeigt; die Nutzung bleibt auch bei laufendem Spiel möglich. Gegebenenfalls sind ein Spielneustart und erneutes Speichern erforderlich. Verwende ein lokales Spielprofil mit deaktivierter Steam-Cloud-Synchronisierung.


[Deutsch Version](README.de.md) | [English Version](README.en.md) | [Portuguese Version](README.pt-br.md) 🔴

Welcome to the ETS2 Save Editor – Alex Edition, a powerful yet user-friendly tool to modify your Euro Truck Simulator 2 profiles, truck settings, and game preferences.
This project is developed as a hobby project, fully open-source, and intended for learning and customization purposes.

# 🚀 Features
## 🚛 Truck Tab

- Repair Truck – Instantly fix your vehicle (Under Development).
- Fuel Level – Adjust fuel levels (Coming Soon).
- Telemetry – View current truck mileage and license plate info (Editing coming in future updates).

📦 Trailer Tab
> [!NOTE] 
> Trailer Management – Main structure implemented. Advanced editing features will be added in upcoming releases.

## 👤 Profile Tab

- Economy – Edit your money and XP values.
- Skills – Manually overwrite driver skill levels.
- Stats – Read-only view of your career progress.

## ⚙️ Settings Tab

- Toggle game-wide settings including:
- Convoy Mode
- Traffic Density
- Parking Doubles
- Developer Mode

> [!NOTE] 
> Changes require a game restart or should be applied while the game is closed.

## 🛠 Installation & Usage

- Download – Visit the GitHub Releases [GitHub Releases](https://github.com/xLieferant/Save-Edit-Tool/releases)
- Install – Download the latest .msi or .exe installer and run it.
- Refresh Profiles – Open the tool and click the Refresh button to load your profiles.
- Edit Values – Modify your desired values (money, XP, skills, etc.) and save.
> [!TIP]
> The tool features an auto-updater. You will be notified automatically when a new version is available.

## ⚠️ Important Notes

***Developer Info – This is a hobby project created by Alex. Everything is open-source for anyone curious to learn how it works. The idea is not to reinvent the wheel but to offer a learning tool. Feedback and contributions are welcome!***

> [!TIP]
> Performance – Some operations may have a slight delay due to Windows Defender scans or background processes. Optimizations are ongoing.

> [!Warning]
> Uninstalling – Standard removal via Add or Remove Programs in Windows.

## 💻 Credits

### Alex – Full development, design, and project management.

### DecryptTruck [@CoffeSiberian](https://github.com/CoffeSiberian) 
Used for decrypting ETS2 save files.

### Open-source Libraries & Tools – Tauri
Rust crates, and other dependencies for backend and UI functionality.

### Community Feedback
Thanks to everyone who tested and provided suggestions during development.

<img width="1434" height="934" alt="Screenshot 2025-12-28 102354" src="https://github.com/user-attachments/assets/42938c58-fb40-4ce3-ac80-582d2699c9a9" />
<img width="1435" height="937" alt="Screenshot 2025-12-28 102346" src="https://github.com/user-attachments/assets/1358ec68-bb5c-4b8b-8842-ab60f6257ea9" />
<img width="259" height="242" alt="Screenshot 2025-12-28 100515" src="https://github.com/user-attachments/assets/b6a763aa-363b-413f-8541-82a5362ab9c9" />
<img width="252" height="61" alt="Screenshot 2025-12-28 100513" src="https://github.com/user-attachments/assets/5c6b8131-b738-4725-8ec2-aaa673bf354a" />
<img width="349" height="118" alt="Screenshot 2025-12-28 100510" src="https://github.com/user-attachments/assets/f7876045-f9a6-4a1e-81ab-1170de51b072" />
<img width="1250" height="886" alt="Screenshot 2025-12-28 100507" src="https://github.com/user-attachments/assets/aef67a77-5f5c-4ded-95c7-1703c9aee352" />
<img width="1437" height="933" alt="Screenshot 2025-12-28 102402" src="https://github.com/user-attachments/assets/ee608a41-ec60-46d1-b9c9-21e52c9fa169" />


## UI and localization checks

The shared desktop design tokens are defined in `ets2-tool/src/styles/design_system.css`.
All module pages load this stylesheet after their layout styles. The default theme
uses neutral dark surfaces, subtle glass panels and visible keyboard focus states.
Existing theme choices remain available.

Run these checks from `ets2-tool/` with Node.js:

```powershell
node scripts/check_i18n.mjs
node scripts/check_i18n.mjs --json
node scripts/check_ui.mjs
```

The localization check validates key coverage in all seven languages, literal source
references, translation markers and interpolation placeholders. Its JSON report
also lists additional legacy keys and keys without detected references. Dynamic
key families require review; an unassigned candidate is not proof of unused text.

The UI check uses an installed Chromium browser, set with `CHROME_PATH` when needed.
It checks seven pages in German and English at 900 x 640, 1210 x 800 and 1600 x 1000,
including translation switching, placeholder interpolation, hidden states, page
overflow and enabled-button text contrast. It uses read-only Tauri fixtures and
writes screenshots to a temporary directory. Native file dialogs and real savegame
writes require separate desktop testing.

The implementation report and file list are in [UI_REVIEW.md](ets2-tool/UI_REVIEW.md).

The UI check also covers independent Dark/Light and green accent preferences,
switch and checkbox interaction, garage lists with up to 125 entries, sidebar
accessibility and workspace selection. Run `node scripts/check_ui.mjs --refinement-only`
from `ets2-tool/` for these targeted workflows. See
[UI_REFINEMENT_REVIEW.md](ets2-tool/UI_REFINEMENT_REVIEW.md) for results and limitations.


## ATS / ETS2 save compatibility

The editor separates ATS and ETS2 profile locations, caches, Steam Cloud app IDs and global settings. Money and truck XP are read from the selected `game.sii`; metadata and profile caches do not override these values. ATS displays USD, ETS2 displays EUR without converting the stored balance. Distance display follows the profile's `g_mph` setting; stored odometer values remain kilometers.

Money, truck XP, player skills, vehicle wear, fuel and odometer edits use shared SII handling. Garage purchase, upgrade, capacity changes and empty-garage relinquishment require a validated save layout, complete references and an unchanged save hash. AI operations preserve driver/truck inventories and reject active or unresolved jobs. Garage ownership edits do not debit or credit the bank balance.

Save writes require a selected, stable save belonging to the selected profile, a backup and temporary-file validation before atomic replacement. ETS2 or ATS can remain running; global configuration edits still require the game to be closed. The custom money/XP reset restores the original game bytes if its metadata update fails. Unknown fields and units are retained. ATS city labels come from save tokens rather than the ETS2 city database.

Validation includes synthetic ATS saves with 18,569 units, metadata conflicts, targeted edits, garage roundtrips, malformed arrays, changed hashes, cross-game isolation and rollback. Existing ETS2 and Project Coaches regression tests also run. The referenced private `profile(1).sii`, `game(1).sii` and `info(1).sii` were unavailable; real ATS in-game loading and Steam Cloud synchronization remain unverified. Unknown layouts remain blocked. Coach progression beyond the existing validated XP operation, arbitrary trailer positioning and ATS career-statistics writes remain unsupported.

Checks from `ets2-tool/`:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --bin ets2-tool
node src-tauri/src/features/save_editor/frontend_tests.mjs
node scripts/check_i18n.mjs
node scripts/check_ui.mjs
```

Profile edits reopen and decode the selected original `game.sii` after atomic replacement. Money success messages require a matching backend reload; a failed save selection is cleared and reported. Tauri IPC regression tests cover ScsC decoding, money/XP/skill persistence, untouched slots and fresh application caches for both games. The separate supplied ATS snapshot can be tested without committing private saves by setting `ATS_GAME_REFERENCE` and `ATS_INFO_REFERENCE` and running `cargo test --manifest-path src-tauri/Cargo.toml --bin ets2-tool provided_ats_snapshot_persists -- --ignored --test-threads=1` from `ets2-tool/`.

### Live savegame synchronization (ATS and ETS2)

Save reads are independent of game process and SDK status. The original issue was caused by path-only decrypt/profile caches with no external-change invalidation, a missing file watcher and a refresh action that reset the selected save during profile scanning. Save writes no longer use the blanket game-process guard. Process detection for Career and telemetry remains available.

`features/save_editor/synchronization.rs` uses `notify` with a two-second polling fallback with content hashes for the selected slot, a 750 ms debounce and three bounded read attempts. It detects new, overwritten and deleted save slots plus changes to `game.sii`, `info.sii` and profile metadata in the existing local, Steam and supported custom search locations. Unchanged polling results do not trigger decryption or parsing; the selected slot is also checked by SHA-256 so preserved timestamps cannot hide changed bytes. File size, modification time and source bytes are checked before publishing a parsed snapshot. Temporary locks or incomplete files keep the last successful data.

Automatic events and the refresh button use the same read-only service. Profiles and saves are rescanned without changing the active selection. Save and vehicle caches are invalidated after validation; money, XP, skills, vehicles, job context and the active dependent view update without a page reload. Active tabs, filters and scroll positions are retained. Open dialogs and unsaved inputs defer synchronization and show a conflict notice. The loaded hash is accepted only after the UI accepts the revision; external-change validation, backups, atomic writes, post-write checks, rollbacks and ATS write-capability gates remain in force.

Synchronization and write feedback use `save_sync.*` and `save_errors.*` keys in all seven locale files. Missing keys, English copies and translation markers are translated with placeholders retained. The checker treats missing keys and markers as errors in every language; additional existing Portuguese keys remain available.

Files changed for synchronization (existing unrelated working-tree changes are preserved):

- Backend: `ets2-tool/src-tauri/src/features/save_editor/synchronization.rs`, `synchronization_tests.rs`, `mod.rs`; `features/profile_manager/commands.rs`; `features/save_analysis/reader.rs`; `src-tauri/src/main.rs`, `state.rs`, `shared/decrypt.rs`.
- Frontend and browser regressions: `ets2-tool/src/main.js`, `src/app.js`, `src-tauri/src/features/save_editor/frontend_tests.mjs`.
- Dependency and locale integration: `ets2-tool/src-tauri/Cargo.toml`, `Cargo.lock`, and `locales/{en,de,es,fr,it,pl,pt-BR}.json`.
- Documentation: `README.md` and `README.en.md`. No version change, release or push is performed.

Regression checks from `ets2-tool/`:

```sh
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml --bin ets2-tool synchronization_tests -- --test-threads=1
node src-tauri/src/features/save_editor/frontend_tests.mjs
node src-tauri/src/features/logging/frontend_tests.mjs
node scripts/check_ui.mjs
node scripts/check_i18n.mjs
```

The new Rust tests cover both games without an SDK, native watching and polling, burst debouncing, slot creation/deletion, metadata changes, custom paths, temporary Windows sharing violations, partial-write recovery, retained caches and Tauri IPC revision/selection conflicts. Browser fixtures cover shared manual/event refresh, updated values, skills and vehicle/job context, draft preservation, serialized refreshes, transient errors, profile switches and retained garage filters in DE/EN.

Regression coverage includes both games, repeated same-path quicksave replacements with identical size and timestamp, actual Windows sharing violations, optimistic write conflicts, rollback preservation, money readback and custom-reset backup/undo through Tauri IPC.

The debug build succeeds. A `cargo tauri dev` smoke test starts a responding app window and logs watcher startup; the test window is closed afterward. Startup also reports unchanged Tauri NPM/Rust plugin-version mismatches and a Career database initialization error (`near "DO": syntax error`). These findings are outside the synchronization change. The smoke test does not verify live in-game saves.

Manual in-game validation remains required:

- Start ATS before the tool and after the tool; repeat for ETS2, including operation without the SDK.
- Create, overwrite and delete manual saves and autosaves while the game runs; repeat rapid saves and manual refresh.
- Switch profiles and games; exercise local profiles, Steam profiles and each supported custom data path.
- Change a save externally while an edit modal contains unsaved input; verify the conflict notice, retained input and refresh after closing it.
- Verify money, XP/level, skills, trucks, trailers, garages, drivers, statistics and active jobs against the selected in-game save.
- Exercise a prolonged file lock or incomplete save, then verify recovery and unchanged save bytes after automatic refresh.

### Editing while the game runs

Create a quicksave or overwrite a manual save in ETS2 or ATS, wait for saving to finish, then select or refresh that exact slot in the tool. Apply the edit and reload the modified slot in the running game. Neither application needs restarting. Open dialogs and unsaved inputs defer automatic refresh; close them before accepting an external save revision.

Decrypt and profile caches validate file size, modification time and SHA-256 content. In-flight reads cannot publish an obsolete generation or a different selection. Each write rereads the current source, validates it, creates a backup, verifies a temporary file in the same directory, rechecks the original before each bounded retry and verifies the replaced file. Five I/O attempts use exponential backoff. A permanent lock or intervening change fails explicitly. Automatic rollback only restores the tool's own written content and preserves external changes.

Run the additional IPC feedback regressions from `ets2-tool/`:

```sh
node src-tauri/src/features/save_editor/write_feedback_tests.mjs
```

Manual reload tests with running ETS2 and ATS, including local and Steam Cloud profiles, remain required. Automated fixture tests do not prove that the running game reloads every supported save layout.

### Bugfix file scope and localization

The running-game restriction came from the unconditional process check in the shared save source reader and additional guards in garage, vehicle-switch and restore transactions. Decrypt-cache hits previously depended on the path alone, and atomic replacement did not retry sharing violations. These checks now distinguish save files from global configuration, validate source revisions and preserve external changes during automatic rollback.

Files changed for this bugfix, relative to the working tree at the start of this task:

- Frontend: `ets2-tool/src/main.js`, `ets2-tool/src/js/applySetting.js`, `ets2-tool/src/modules/shared/diagnostics_runtime.js`.
- Shared backend: `ets2-tool/src-tauri/src/state.rs`, `ets2-tool/src-tauri/src/shared/decrypt.rs`.
- Save editor: `ets2-tool/src-tauri/src/features/save_editor/{commands.rs,safety.rs,synchronization.rs,synchronization_tests.rs,persistence_ipc_tests.rs,frontend_tests.mjs,write_feedback_tests.mjs}`.
- Transactions: `ets2-tool/src-tauri/src/features/backup/{commands.rs,service.rs}`, `features/ets2save/sii_codec.rs`, `features/garages/{commands.rs,service.rs,writer.rs}`, `features/health_monitor/{commands.rs,service.rs}`, `features/settings/apply_settings.rs`, `features/truck_change/{service.rs,writer.rs}`, `features/trailer_change/{service.rs,writer.rs}` (all feature paths are under `ets2-tool/src-tauri/src/`).
- Cache publication: `ets2-tool/src-tauri/src/features/save_analysis/{reader.rs,quicksave.rs}`, `features/vehicles/{trucks.rs,trailers.rs}`.
- Localization and checks: `ets2-tool/src-tauri/locales/{en,de,es,fr,it,pl,pt-BR}.json`, `ets2-tool/scripts/check_i18n.mjs`, `ets2-tool/scripts/check_ui.mjs`.
- Documentation: `README.md`, `README.en.md`.
- Formatting only: `ets2-tool/src-tauri/src/main.rs`, `shared/{paths.rs,sii_parser.rs}`, `features/{garages/parser.rs,logging/mod.rs,mod_profile_manager/sandbox.rs,profile_manager/commands.rs,settings/ets2_paths.rs,settings/game_config.rs,truck_change/parser.rs,vehicles/editor.rs}`.

Localization counts include new error keys and previously missing, marked or English-copy values, measured against the initial working tree. English counts source additions. Existing keys and JSON containers remain in place.

| Language | Entries added or changed | New keys | Remaining markers |
| --- | ---: | ---: | ---: |
| English | 11 | 11 | 0 |
| German | 46 | 11 | 0 |
| Spanish | 988 | 778 | 0 |
| French | 987 | 778 | 0 |
| Italian | 988 | 778 | 0 |
| Polish | 994 | 778 | 0 |
| Brazilian Portuguese | 2763 | 2555 | 0 |

All seven languages contain the 2778 English reference keys. The Portuguese file retains 760 additional legacy keys; the checker reports these as extra-key warnings. Missing-key, empty-value, marker and placeholder checks pass. External translation of UI text was explicitly authorized; technical identifiers and placeholders were protected locally, with local corrections to product names and domain terminology.

Final automated validation passes: `cargo fmt --check`, `cargo check`, `cargo test` (352 passed, two existing ignored tests), `cargo build`, both `check_i18n.mjs` variants, `check_ui.mjs`, the Save Editor browser regressions and 16 frontend unit tests. The general browser suite covers 42 page/language/window fixtures, 56 theme/accent checks and 96 garage volume/window checks. The modal fixture waits for the existing app-ready event and finite CSS transitions before asserting initialized values and final text contrast.

The `cargo tauri dev --no-watch` smoke test opens a responding native window and closes it cleanly. Tauri still reports the existing NPM/Rust FS and updater plugin version mismatches; dependencies are not upgraded for this fix. Manual in-game reloads with actually running ETS2 and ATS, including local and Steam Cloud profiles, remain pending. Automated process probes and save fixtures do not substitute for those tests.
