# Performance notes

- **Profile cache layer**: Added `ProfileCache` to `src/state.rs` and wired it into all `read_*` commands (`read_all_save_data`, `quicksave_game_info`, `read_base_config`, `read_save_config`, `get_all_trucks`, `get_player_truck`, `get_all_trailers`, `get_player_trailer`). All cache hits now reuse decrypted/parsing results, and cache entries are cleared whenever the user switches profiles/saves or hits an editor command.
- **Cache invalidation hooks**: Stated commands that mutate saves or configs (`save_editor`, `vehicles::editor`, `apply_setting`, `profile_manager` setters) now call cache invalidation helpers so both backend and frontend see the updated data immediately.
- **Shared decrypt cache usage**: Vehicle helpers now call `decrypt_cached` so multiple parsers reuse decrypted bytes.
- **User action logging**: Implemented `log_user_action` on the backend and wired frontend helpers to log start/success/error points for scan/load/clone/move-mods operations. Logs are written to `ets2_tool_user.log`.
- **Frontend UX notes**: Added `window.logUserAction` and systematically call it from `main.js` (profile/save scans, clone, move/copy mods) and from `app.js` (clone modal).
- **Testing checklist**:
  1. `cargo check` from `src-tauri` to confirm the new cache state compiles.
  2. `cargo tauri dev` to manually verify profile/save scanning still works, the tool reflects edited values immediately, and the new log file `ets2_tool_user.log` records actions.
  3. Open the UI and perform a profile clone, move mods, and a save load to confirm caches invalidate and no stale values persist.
