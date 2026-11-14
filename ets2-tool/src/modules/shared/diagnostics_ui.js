import { t } from "./i18n.js";
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
  const keys = ["title", "description", "ready", "loading", "load_error", "path", "size", "warnings", "error_count", "updated", "all", "info", "viewer", "empty", "export", "exporting", "exported", "kb", "mb"];
  const copy = Object.fromEntries(await Promise.all(keys.map(async (key) => [key, await t(`user_log_report.${key}`)])));
  const previousFocus = document.activeElement;
  const state = { entries: [], filtered: [], rendered: 0, filter: "all", revision: null, loading: false, exporting: false, closed: false, timer: null };
  const dialog = await element("dialog", null, "log-file-dialog");
  dialog.setAttribute("aria-labelledby", "log-file-title");
  const header = await element("header", null, "log-file-head");
  const heading = await element("div");
  const title = await element("h2", "user_log_report.title");
  title.id = "log-file-title";
  const description = await element("p", "user_log_report.description");
  heading.append(title, description);
  const status = await element("span", "user_log_report.loading", "log-file-status");
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  header.append(heading, status);

  const metadata = await element("section", null, "log-file-metadata");
  const values = {};
  for (const key of ["path", "size", "warnings", "errors"]) {
    const card = await element("article", null, "log-file-meta-card");
    const label = await element("span", `user_log_report.${key === "errors" ? "error_count" : key}`, "log-file-meta-label");
    values[key] = await element("strong", null, key === "path" ? "log-file-path" : "");
    values[key].textContent = "-";
    values[key].dataset.logMeta = key;
    card.append(label, values[key]);
    metadata.append(card);
  }
  const updated = await element("p", null, "log-file-updated");
  const updatedLabel = await element("span", "user_log_report.updated");
  const updatedValue = await element("time");
  updatedValue.textContent = "-";
  updated.append(updatedLabel, document.createTextNode(": "), updatedValue);

  const filters = await element("div", null, "log-file-filters");
  filters.setAttribute("role", "tablist");
  filters.setAttribute("aria-label", copy.viewer);
  const filterButtons = [];
  for (const [filter, key] of [["all", "all"], ["INFO", "info"], ["WARN", "warnings"], ["ERROR", "error_count"]]) {
    const button = await element("button", `user_log_report.${key}`);
    button.type = "button";
    button.dataset.logFilter = filter;
    button.setAttribute("role", "tab");
    button.setAttribute("aria-controls", "user-log-records");
    button.addEventListener("click", () => {
      state.filter = filter;
      renderEntries();
    });
    button.addEventListener("keydown", (event) => {
      if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
      event.preventDefault();
      const index = filterButtons.indexOf(button);
      const target = event.key === "Home" ? 0 : event.key === "End" ? 3 : (index + (event.key === "ArrowLeft" ? 3 : 1)) % 4;
      filterButtons[target].click();
      filterButtons[target].focus();
    });
    filterButtons.push(button);
    filters.append(button);
  }
  const preview = await element("div", null, "log-file-preview");
  preview.id = "user-log-records";
  preview.tabIndex = 0;
  preview.setAttribute("role", "tabpanel");
  preview.setAttribute("aria-label", copy.viewer);
  const feedback = await element("p", null, "log-file-feedback");
  feedback.setAttribute("role", "status");
  feedback.setAttribute("aria-live", "polite");
  const footer = await element("footer", null, "log-file-footer");
  const actions = await element("div", null, "log-file-actions");
  const exportButton = await element("button", "user_log_report.export", "apply log-file-export");
  exportButton.type = "button";
  const closeButton = await element("button", "simple_logging.close");
  closeButton.type = "button";
  closeButton.addEventListener("click", () => dialog.close());
  actions.append(exportButton, closeButton);
  footer.append(feedback, actions);
  dialog.append(header, metadata, updated, filters, preview, footer);

  function setStatus(key, failed = false) {
    status.textContent = copy[key];
    status.dataset.state = failed ? "error" : key === "loading" ? "loading" : "ready";
  }

  function appendEntries() {
    const fragment = document.createDocumentFragment();
    const next = Math.min(state.rendered + 250, state.filtered.length);
    for (; state.rendered < next; state.rendered += 1) {
      const entry = state.filtered[state.rendered];
      const row = document.createElement("pre");
      row.className = "log-file-entry";
      row.dataset.level = entry.level || "unknown";
      row.textContent = entry.text;
      fragment.append(row);
    }
    preview.append(fragment);
  }

  function renderEntries(preserveScroll = false) {
    const scroll = preserveScroll ? preview.scrollTop : 0;
    state.filtered = state.entries.filter((entry) => state.filter === "all" || entry.level === state.filter
      || (state.filter === "WARN" && entry.level === "WARNING") || (state.filter === "ERROR" && entry.level === "FATAL"));
    preview.replaceChildren();
    state.rendered = 0;
    for (const button of filterButtons) {
      const selected = button.dataset.logFilter === state.filter;
      button.setAttribute("aria-selected", String(selected));
      button.tabIndex = selected ? 0 : -1;
    }
    if (!state.filtered.length) {
      const empty = document.createElement("p");
      empty.className = "log-file-empty";
      empty.textContent = copy.empty;
      preview.append(empty);
    } else {
      appendEntries();
      while (state.rendered < state.filtered.length && preview.scrollHeight < scroll + preview.clientHeight) appendEntries();
    }
    preview.scrollTop = scroll;
  }

  function handleScroll() {
    if (preview.scrollTop + preview.clientHeight >= preview.scrollHeight - 300) appendEntries();
  }
  preview.addEventListener("scroll", handleScroll);

  async function refresh() {
    if (state.loading || state.exporting || state.closed) return;
    state.loading = true;
    if (!state.revision) setStatus("loading");
    try {
      const snapshot = await invoke("diagnostics_user_logs", { knownRevision: state.revision });
      if (state.closed) return;
      if (snapshot) {
        state.revision = snapshot.revision;
        state.entries = snapshot.entries;
        values.path.textContent = snapshot.logFilePath;
        values.path.title = snapshot.logFilePath;
        const bytes = Math.max(0, Number(snapshot.sizeBytes) || 0);
        const megabytes = bytes >= 1024 * 1024;
        values.size.textContent = `${new Intl.NumberFormat(document.documentElement.lang, { maximumFractionDigits: 2 }).format(bytes / (megabytes ? 1024 * 1024 : 1024))} ${megabytes ? copy.mb : copy.kb}`;
        values.warnings.textContent = String(snapshot.warningCount);
        values.errors.textContent = String(snapshot.errorCount);
        const modified = snapshot.lastModified ? new Date(snapshot.lastModified) : null;
        updatedValue.textContent = modified && Number.isFinite(modified.getTime())
          ? new Intl.DateTimeFormat(document.documentElement.lang, { dateStyle: "short", timeStyle: "medium" }).format(modified) : "-";
        if (modified && Number.isFinite(modified.getTime())) updatedValue.dateTime = modified.toISOString();
        renderEntries(true);
      }
      setStatus("ready");
    } catch {
      if (state.closed) return;
      setStatus("load_error", true);
      if (!state.revision) preview.textContent = copy.load_error;
    } finally {
      state.loading = false;
    }
  }

  async function poll() {
    if (!state.closed && document.visibilityState !== "hidden") await refresh();
    if (!state.closed) state.timer = setTimeout(poll, 5000);
  }
  const handleVisibility = () => { if (document.visibilityState !== "hidden") void refresh(); };
  document.addEventListener("visibilitychange", handleVisibility);

  exportButton.addEventListener("click", async () => {
    if (state.exporting) return;
    state.exporting = true;
    exportButton.disabled = true;
    closeButton.disabled = true;
    exportButton.textContent = copy.exporting;
    feedback.textContent = "";
    feedback.dataset.state = "ready";
    try {
      // The backend reads the full source independently of the selected UI filter.
      const path = await invoke("export_log_file");
      if (path) {
        feedback.textContent = copy.exported;
        feedback.title = path;
        window.showToast?.("user_log_report.exported", {}, "success");
      }
    } catch (error) {
      const key = String(error?.message || error);
      feedback.textContent = await t(key.startsWith("user_log_report.errors.") || key.startsWith("diagnostics.errors.") ? key : "user_log_report.errors.write_failed");
      feedback.dataset.state = "error";
      void recordDiagnostic({ level: "ERROR", module: "logging", action: "export", message: "User log report export failed.", technical: String(error) });
    } finally {
      state.exporting = false;
      exportButton.disabled = false;
      closeButton.disabled = false;
      exportButton.textContent = copy.export;
    }
  });
  dialog.addEventListener("cancel", (event) => { if (state.exporting) event.preventDefault(); });
  dialog.addEventListener("close", () => {
    state.closed = true;
    clearTimeout(state.timer);
    document.removeEventListener("visibilitychange", handleVisibility);
    preview.removeEventListener("scroll", handleScroll);
    dialog.remove();
    previousFocus?.focus();
  }, { once: true });
  document.body.append(dialog);
  renderEntries();
  dialog.showModal();
  closeButton.focus();
  await refresh();
  if (!state.closed) state.timer = setTimeout(poll, 5000);
}

export async function clearTemporaryCache() {
  try {
    const preview = await invoke("diagnostics_cleanup_preview", { scope: "cache" });
    if (!window.confirm(await t("simple_logging.confirm_cache"))) return;
    await invoke("diagnostics_cleanup", { scope: "cache", token: preview.token, confirmed: true });
    window.showToast?.("simple_logging.cache_cleared", {}, "success");
  } catch (error) {
    void recordDiagnostic({ level: "WARN", module: "cache", action: "clear", message: "Temporary cache could not be cleared.", technical: String(error) });
    window.showToast?.("simple_logging.errors.cache_failed", {}, "error");
  }
}
