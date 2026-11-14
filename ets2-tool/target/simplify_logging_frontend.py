exec((__import__('pathlib').Path(__file__).with_name('simplify_logging.py')).read_text(encoding='utf-8').split('old = ')[0])
write(root / 'src/modules/shared/diagnostics_runtime.js', r'''const rawInvoke = window.__TAURI__?.core?.invoke;
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
''')
write(root / 'src/modules/shared/diagnostics_ui.js', '''import { t } from "./i18n.js";
import { recordDiagnostic } from "./diagnostics_runtime.js";

const invoke = (...args) => window.__TAURI__.core.invoke(...args);
let opening = null;

async function element(tag, key, className) {
  const node = document.createElement(tag);
  if (key) { node.dataset.translate = key; node.textContent = await t(key); }
  if (className) node.className = className;
  return node;
}

export function openLogFile() {
  if (opening) return opening;
  const existing = document.querySelector("dialog.log-file-dialog[open]");
  if (existing) { existing.focus(); return Promise.resolve(); }
  opening = showLogFile().finally(() => { opening = null; });
  return opening;
}

async function showLogFile() {
  if (!document.getElementById("log-file-styles")) {
    const stylesheet = document.createElement("link");
    stylesheet.id = "log-file-styles";
    stylesheet.rel = "stylesheet";
    stylesheet.href = "/modules/shared/diagnostics.css";
    document.head.append(stylesheet);
  }
  const previousFocus = document.activeElement;
  const dialog = await element("dialog", null, "log-file-dialog");
  dialog.setAttribute("aria-labelledby", "log-file-title");
  const title = await element("h2", "simple_logging.show");
  title.id = "log-file-title";
  const description = await element("p", "simple_logging.preview_description");
  const preview = await element("pre", null, "log-file-preview");
  preview.tabIndex = 0;
  preview.setAttribute("aria-label", await t("simple_logging.preview_description"));
  const status = await element("p", "simple_logging.loading", "log-file-status");
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  const actions = await element("div", null, "log-file-actions");
  const exportButton = await element("button", "simple_logging.export");
  exportButton.type = "button";
  exportButton.disabled = true;
  const closeButton = await element("button", "simple_logging.close");
  closeButton.type = "button";
  closeButton.addEventListener("click", () => dialog.close());
  exportButton.addEventListener("click", async () => {
    exportButton.disabled = true;
    try {
      const path = await invoke("export_log_file");
      status.textContent = path ? await t("simple_logging.exported") : "";
    } catch (error) {
      status.textContent = await t("simple_logging.errors.export_failed");
      void recordDiagnostic({ level: "ERROR", module: "logging", action: "export", message: "TXT log export failed.", technical: String(error) });
    } finally { exportButton.disabled = false; }
  });
  dialog.addEventListener("close", () => { dialog.remove(); previousFocus?.focus(); }, { once: true });
  actions.append(exportButton, closeButton);
  dialog.append(title, description, preview, status, actions);
  document.body.append(dialog);
  dialog.showModal();
  closeButton.focus();
  try {
    preview.textContent = await invoke("get_log_preview") || await t("simple_logging.empty");
    preview.scrollTop = preview.scrollHeight;
    status.textContent = "";
    exportButton.disabled = false;
  } catch {
    status.textContent = await t("simple_logging.errors.load_failed");
  }
}

export async function clearTemporaryCache() {
  try {
    const preview = await invoke("diagnostics_cleanup_preview", { scope: "cache" });
    if (!window.confirm(await t("simple_logging.confirm_cache"))) return;
    await invoke("diagnostics_cleanup", { scope: "cache", token: preview.token, confirmed: true });
    window.showToast?.("simple_logging.cache_cleared", {}, "success");
  } catch {
    window.showToast?.("simple_logging.errors.cache_failed", {}, "error");
  }
}
''')
write(root / 'src/modules/shared/diagnostics.css', '''.log-file-dialog {
  box-sizing: border-box;
  width: min(720px, calc(100vw - 32px));
  max-height: calc(100vh - 48px);
  padding: 24px;
  border: 1px solid var(--panel-border, #363e4b);
  border-radius: 12px;
  background: var(--bg-card, #1b202a);
  color: var(--text-main, #eef0f5);
  font-family: var(--font-ui, sans-serif);
}
.log-file-dialog::backdrop { background: rgb(0 0 0 / 55%); }
.log-file-dialog h2 { margin: 0 0 12px; font-size: 1.15rem; }
.log-file-dialog p { font-size: .9rem; }
.log-file-preview {
  box-sizing: border-box;
  height: min(340px, 45vh);
  overflow: auto;
  padding: 12px;
  border: 1px solid var(--panel-border, #363e4b);
  border-radius: 6px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  font: 12px/1.5 var(--font-mono, monospace);
  background: rgb(0 0 0 / 12%);
}
.log-file-status { min-height: 1.3em; }
.log-file-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 10px; }
.log-file-actions button {
  padding: 9px 15px;
  border: 1px solid var(--panel-border, #465164);
  border-radius: 6px;
  background: var(--bg-card, #252e3d);
  color: inherit;
  font: inherit;
  cursor: pointer;
}
.log-file-actions button:disabled { opacity: .55; cursor: wait; }
.log-file-dialog :focus-visible { outline: 2px solid var(--accent, #8ba9ff); outline-offset: 3px; }
''')
p = root / 'src/tools.js'
s = p.read_text(encoding='utf-8')
start = s.index('    {\n      title: "diagnostics.title"')
end = s.index('    {', start + 5)
s = s[:start] + s[end:]
s = s.replace('title: "tools.settings.user_logs.title"', 'title: "simple_logging.show"').replace('desc: "tools.settings.user_logs.desc"', 'desc: "simple_logging.summary"')
start = s.index('    {\n      title: "tools.settings.ets2_data_location.title"')
s = s[:start] + '''    {
      title: "simple_logging.clear_cache",
      desc: "simple_logging.cache_summary",
      img: "images/dev.jpg",
      action: async () => {
        const { clearTemporaryCache } = await import("./modules/shared/diagnostics_ui.js");
        await clearTemporaryCache();
      },
      disabled: false,
    },
''' + s[start:]
write(p, s)
p = root / 'src/main.js'
s = p.read_text(encoding='utf-8').replace('const { openDiagnostics } = await import("./modules/shared/diagnostics_ui.js");', 'const { openLogFile } = await import("./modules/shared/diagnostics_ui.js");').replace('await openDiagnostics();', 'await openLogFile();')
write(p, s)

# Byte insertion preserves existing mixed line endings and all existing translations.
import json
en = {
 'show': 'Show log file', 'summary': 'Preview recent log entries and export the complete TXT file.',
 'export': 'Export log file', 'close': 'Close', 'preview_description': 'Last 100 log lines',
 'loading': 'Loading log file...', 'empty': 'No log entries yet.', 'exported': 'Log file exported.',
 'clear_cache': 'Clear cache', 'cache_summary': 'Remove temporary application cache files.',
 'confirm_cache': 'Clear temporary application cache files? Savegames, profiles, backups and settings are preserved.',
 'cache_cleared': 'Temporary cache cleared.',
 'errors': {'load_failed': 'The log file could not be loaded.', 'export_failed': 'The log file could not be exported.', 'cache_failed': 'The cache could not be cleared. Please try again.', 'txt_only': 'Choose a .txt file.'}
}
de = {k: ('[NEEDS TRANSLATION] ' + v if isinstance(v, str) else {a: '[NEEDS TRANSLATION] ' + b for a, b in v.items()}) for k, v in en.items()}
de.update(show='Logdatei anzeigen', export='Logdatei exportieren', close='Schließen', clear_cache='Cache löschen')
for language, values in [('en', en), ('de', de)]:
    p = root / f'src-tauri/locales/{language}.json'
    data = p.read_bytes()
    index = data.rfind(b'}')
    block = json.dumps({'simple_logging': values}, ensure_ascii=False, indent=2)[1:-1].strip('\n')
    data = data[:index].rstrip() + b',\n' + block.encode('utf-8') + b'\n' + data[index:]
    p.write_bytes(data)

for name in ['README.md', 'README.en.md']:
    p = root / name
    s = p.read_text(encoding='utf-8')
    start = s.index('## Errors & Diagnostics')
    end = s.index('## ETS2 Data Location', start)
    s = s[:start] + '''## Application logs

Settings provides one **Show log file** button with a scrollable preview of the
last 100 lines, **Export log file** and **Close**. Export uses the native Save
dialog and writes the complete current `.txt` log. **Clear cache** only removes
files in the dedicated temporary application cache.

Rust and JavaScript errors are recorded locally in
`<Tauri local application data>/support/logs/application.log.txt`. Entries append
across restarts, include operation identifiers and redact private paths and known
secret formats. The background writer rotates at 5 MiB; managed TXT archives
expire after 14 days and the total managed log size is limited to 20 MiB.
No reports are uploaded automatically. Existing legacy reports remain untouched.

Implementation and tests: [Logging](src-tauri/src/features/logging/README.md).

''' + s[end:]
    write(p, s)
write(backend / 'README.md', '''# Local application logging

The existing shared logger routes Rust `log` records, legacy user logs, verified
save-operation events and frontend failures to one bounded background writer.
There are three persisted levels: INFO, WARN and ERROR. Ordinary DEBUG/TRACE data
is excluded; existing trace errors are promoted to ERROR. Records contain a local
timestamp with milliseconds and offset, module/action, session/operation IDs,
message, technical cause and available safe context. No raw IPC arguments or save
contents are collected. Existing IPC results and rejections are preserved.

## Files and limits

`support/logs/application.log.txt` is below Tauri's local application data path.
The file is appended across restarts and rotated before exceeding 5 MiB. Managed
archives end in `.txt`, expire after 14 days and share a 20 MiB cap with the active
log. A panic tries to flush the writer, then writes synchronously; if the writer
is busy or panicked, a separate emergency TXT archive preserves the panic.
An abruptly terminated process or unavailable disk can still lose unwritten data.
Queue overload is bounded and counted rather than blocking normal application work.
Old JSONL/PDF/ZIP reports are neither created nor automatically migrated/deleted.

The settings modal displays the last 100 physical log lines. Native Save exports
the full current TXT file using an atomic replacement; it cannot replace internal
application data. Canceling the dialog does not create a file. The existing
`export_user_logs`, `export_logs_bundle` and `build_support_report` adapters now
use this TXT log as well. No email or network transfer is performed.

## Privacy and cache boundaries

Text is redacted before entering the writer or in-memory history: private paths,
emails, named passwords/tokens, bearer credentials and common JWTs. Raw save
content is discarded. Redaction is conservative and cannot recognize every
possible unlabelled secret; inspect the TXT file before sharing it manually.
Cache cleanup only visits `support/cache`, with a matching inventory token and
explicit user confirmation. Traversal, symlinks and Windows reparse points are
rejected, including ancestors. Profiles, savegames, backups and settings outside
that dedicated temporary directory are never cleanup candidates.

## Verification

From `ets2-tool`:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --lib shared::logs::diagnostic
cargo test --manifest-path src-tauri/Cargo.toml --bin ets2-tool features::logging
node --test src-tauri/src/features/logging/frontend_tests.mjs
cargo tauri dev --no-watch
```

`ui_smoke.mjs` checks the real Tauri WebView (CDP port 19223) in English and
German. It inserts a synthetic local event and changes/restores the UI language;
it never opens or modifies game saves. German labels supplied by the user are
used verbatim; other additions retain the project's `[NEEDS TRANSLATION]` marker.
''')
