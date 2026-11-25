const rawInvoke = window.__TAURI__?.core?.invoke;
const state = window.__ETS2_DIAGNOSTICS__ ||= {
  installed: false, pending: 0, events: new Map(), handled: new WeakMap(),
};
const internal = (command) => /^(diagnostics_|get_log_preview$|export_log_file$|log_|get_user_logs$|get_log_status$|translate_command|get_current_language_command|get_available_languages_command)/.test(command);
const trackedAction = (command) => /^(load_profile|select_|set_|save_|apply_|write_|create_|restore_|delete_|remove_|change_|purchase_|hire_|decrypt_|export_)/.test(command);
const operationId = () => `OP-${globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(16).slice(2)}`}`;
const errorText = (error) => error instanceof Error ? error.message : typeof error === "string" ? error.slice(0, 8192) : "Non-text error; details unavailable";

const savegameCommands = new Set([
  "edit_money", "edit_player_money", "edit_xp", "edit_level", "edit_career_xp", "edit_player_experience", "edit_skill_value",
  "apply_custom_reset_values", "undo_last_save_change", "restore_backup", "apply_save_health_fix",
  "repair_player_truck", "repair_player_trailer", "refuel_player_truck", "set_player_truck_fuel", "set_player_truck_wear",
  "edit_truck_odometer", "set_player_truck_license_plate", "set_player_trailer_license_plate", "set_player_trailer_cargo_mass",
  "purchase_garage", "buy_all_garages", "upgrade_owned_garage", "update_garage", "relinquish_empty_garages",
  "relinquish_garage_ownership", "assign_random_garage_resources", "generate_ai_drivers", "update_ai_driver_truck", "remove_ai_drivers",
  "apply_active_truck_switch", "apply_active_trailer_switch",
]);
const editsSavegame = (command, args) => savegameCommands.has(command)
  || (command === "apply_setting" && ["money", "xp", "adr", "long_dist", "heavy", "fragile", "urgent", "mechanical"].includes(args.payload?.key));
const saveReadCommands = new Set(["read_all_save_data", "quicksave_game_info", "read_project_coaches", "get_all_trucks", "get_player_truck", "get_all_trailers", "get_player_trailer", "get_current_truck_summary"]);

export function saveErrorKey(error) {
  const text = errorText(error);
  if (/^save_errors\.[a-z_]+$/.test(text)) return text;
  if (/save_changed|save_selection_changed/.test(text)) return "save_errors.changed";
  if (/not_found|save_not_loaded|profile_not_loaded|save.*missing/.test(text)) return "save_errors.not_found";
  if (/backup|undo.snapshot|Automatisches Backup/.test(text)) return "save_errors.backup_failed";
  if (/decode_failed|decrypt/.test(text)) return "save_errors.decrypt_failed";
  if (/verif|validation|rollback/.test(text)) return "save_errors.verification_failed";
  if (/locked|sharing.violation|os error (32|33)/i.test(text)) return "save_errors.locked";
  if (/save_mutation_in_progress/.test(text)) return "save_errors.busy";
  if (/save_(read|write|temporary|atomic|post_write)|restore_(replace|target_unreadable)/.test(text)) return "save_errors.write_failed";
  return null;
}

function errorCauses(error) {
  const causes = [];
  const seen = new Set();
  let current = error instanceof Error ? error.cause : null;
  while (current && causes.length < 8 && !seen.has(current)) {
    seen.add(current);
    causes.push(errorText(current));
    current = current instanceof Error ? current.cause : null;
  }
  return causes;
}

export function isExpectedError(command, error) {
  const text = errorText(error).toLowerCase();
  const expected = ["no_profile_selected", "no_save_selected", "no profile selected", "no save selected", "not authenticated", "garage_mutation_in_progress", "select a custom ets2 data location first", "owned_trailers_missing", "active_trailer_not_found"];
  return expected.some((value) => text === value || text.startsWith(`${value}:`))
    || /^(?:kein (?:profil|save)|save nicht gefunden)/.test(text)
    || (/^(find_|get_ets2_data_location|set_ets2_data_location_mode)/.test(command)
      && /(?:folder|directory|path).*(?:not found|does not exist|missing)/i.test(text));
}

export async function recordDiagnostic(event) {
  if (!rawInvoke || state.pending >= 32) return null;
  state.pending += 1;
  try { return await rawInvoke("diagnostics_record", { event }); }
  catch { return null; }
  finally { state.pending -= 1; }
}

function remember(error) {
  const now = Date.now();
  if (error && typeof error === "object") state.handled.set(error, now);
  state.events.set(errorText(error), now);
  for (const [key, time] of state.events) if (time < now - 5000) state.events.delete(key);
  if (state.events.size > 128) state.events.delete(state.events.keys().next().value);
}
function alreadyRecorded(error) {
  const time = error && typeof error === "object" ? state.handled.get(error) : state.events.get(errorText(error));
  return time !== undefined && time > Date.now() - 5000;
}

export async function diagnosticInvoke(command, args = {}) {
  if (!rawInvoke) throw new Error("Tauri API not available");
  if (internal(command)) return rawInvoke(command, args);
  const id = operationId();
  const started = performance.now();
  const base = { module: "ipc", action: command, operationId: id };
  const sync = editsSavegame(command, args) ? window.saveSync : null;
  const readSync = saveReadCommands.has(command) ? window.saveSync : null;
  let mutationTracked = false;
  if (trackedAction(command)) void recordDiagnostic({ ...base, level: "INFO", message: "Command started." });
  try {
    if (readSync) readSync.reads = (readSync.reads || 0) + 1;
    if (sync?.accepting || sync?.blockWrites) throw "save_sync.conflict";
    if (sync) {
      sync.mutations++;
      mutationTracked = true;
    }
    const result = await rawInvoke(command, { ...args, _diagnosticOperationId: id });
    if (trackedAction(command)) void recordDiagnostic({ ...base, level: "INFO", message: "Command completed.", outcome: "completed",
      context: { durationMs: String(Math.round(performance.now() - started)) } });
    if (editsSavegame(command, args)) {
      void window.showToast?.("save_errors.success", {}, "success");
      window.dispatchEvent(new CustomEvent("savegame-modified", { detail: { command } }));
    }
    return result;
  } catch (error) {
    remember(error);
    const expected = isExpectedError(command, error);
    void recordDiagnostic({ ...base, level: expected ? "WARN" : "ERROR", message: expected ? "Configuration or user action required." : "Command failed.",
      technical: errorText(error), stack: error instanceof Error ? error.stack : null, causes: errorCauses(error),
      errorKind: expected ? "configuration_or_user_input" : "command_failure", outcome: "failed; file state unavailable" });
    const key = editsSavegame(command, args) ? saveErrorKey(error) : null;
    if (key) void window.showToast?.(key, { details: errorText(error) }, "error");
    throw key || error;
  } finally {
    if (mutationTracked) sync.mutations = Math.max(0, sync.mutations - 1);
    if (readSync) readSync.reads = Math.max(0, readSync.reads - 1);
  }
}

function capture(action, error, context = {}, level = "ERROR") {
  if (alreadyRecorded(error)) return;
  remember(error);
  void recordDiagnostic({ level, module: "frontend", action, message: level === "WARN" ? "A JavaScript warning occurred." : "A JavaScript error occurred.",
    technical: errorText(error), stack: error instanceof Error ? error.stack : null, causes: errorCauses(error),
    errorKind: error instanceof Error ? error.name : "javascript_error", context });
}

if (!state.installed && rawInvoke) {
  state.installed = true;
  window.addEventListener("error", (event) => {
    if (event.error || event.message) capture("exception", event.error || event.message, {
      fileName: String(event.filename || "").split(/[\\/]/).pop(), line: String(event.lineno || ""), column: String(event.colno || ""),
    });
  });
  window.addEventListener("unhandledrejection", (event) => capture("unhandled_rejection", event.reason));
  // Existing JS adapters report handled failures through console. Never serialize arbitrary objects.
  for (const [method, level] of [["error", "ERROR"], ["warn", "WARN"]]) {
    const original = console[method].bind(console);
    console[method] = (...args) => {
      original(...args);
      const error = args.find((arg) => arg instanceof Error);
      if (error) capture(`console_${method}`, error, {}, level);
      else {
        const strings = args.filter((arg) => typeof arg === "string").slice(0, 4);
        if (strings.some((value) => alreadyRecorded(value))) return;
        if (strings.length) capture(`console_${method}`, strings.join(" ").slice(0, 8192), {}, level);
      }
    };
  }
}
