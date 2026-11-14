const rawInvoke = window.__TAURI__?.core?.invoke;
const state = window.__ETS2_DIAGNOSTICS__ ||= {
  installed: false, pending: 0, events: new Map(), handled: new WeakMap(),
};
const internal = (command) => /^(diagnostics_|get_log_preview$|export_log_file$|log_|get_user_logs$|get_log_status$|translate_command|get_current_language_command|get_available_languages_command)/.test(command);
const trackedAction = (command) => /^(load_profile|select_|set_|save_|apply_|write_|create_|restore_|delete_|remove_|change_|purchase_|hire_|decrypt_|export_)/.test(command);
const operationId = () => `OP-${globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(16).slice(2)}`}`;
const errorText = (error) => error instanceof Error ? error.message : typeof error === "string" ? error.slice(0, 8192) : "Non-text error; details unavailable";

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
  if (trackedAction(command)) void recordDiagnostic({ ...base, level: "INFO", message: "Command started." });
  try {
    const result = await rawInvoke(command, { ...args, _diagnosticOperationId: id });
    if (trackedAction(command)) void recordDiagnostic({ ...base, level: "INFO", message: "Command completed.", outcome: "completed",
      context: { durationMs: String(Math.round(performance.now() - started)) } });
    return result;
  } catch (error) {
    remember(error);
    const expected = isExpectedError(command, error);
    void recordDiagnostic({ ...base, level: expected ? "WARN" : "ERROR", message: expected ? "Configuration or user action required." : "Command failed.",
      technical: errorText(error), stack: error instanceof Error ? error.stack : null, causes: errorCauses(error),
      errorKind: expected ? "configuration_or_user_input" : "command_failure", outcome: "failed; file state unavailable" });
    throw error;
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
