import { t } from "./i18n.js";
import { frontendState, readPreference, savePreference } from "./frontend_state.js";
import { notify } from "./notifications.js";

const isVisible = (node) => node && !node.disabled && node.getClientRects().length > 0 && getComputedStyle(node).visibility !== "hidden";
const openDialogs = () => [...document.querySelectorAll("[role='dialog'], .modal-backdrop, .modal")].filter(isVisible);
let installed = false;

// Install once per document, including auxiliary pages loaded through analytics.
export function installDesktop() {
  if (installed) return;
  installed = true;
  const controller = new AbortController();
  const options = { signal: controller.signal };
  let resumeTimer;
  let lastResume = 0;
  const resume = () => {
    clearTimeout(resumeTimer);
    if (document.hidden || Date.now() - lastResume < 2000) return;
    resumeTimer = setTimeout(() => {
      lastResume = Date.now();
      Promise.resolve(frontendState.refresh?.()).catch(console.error);
    }, 180);
  };
  window.addEventListener("focus", resume, options);
  document.addEventListener("visibilitychange", resume, options);
  document.addEventListener("keydown", (event) => {
    if (event.defaultPrevented || event.altKey || event.isComposing || event.repeat) return;
    const modifier = event.ctrlKey || event.metaKey;
    const key = event.key.toLowerCase();
    if (event.key === "F5" || (modifier && key === "r")) {
      if (!frontendState.refresh) return;
      event.preventDefault();
      void frontendState.refresh({ manual: true });
      return;
    }
    if (modifier && key === "f") {
      const scope = openDialogs().at(-1) || document.querySelector(".career-panel.is-active") || document;
      const search = [...scope.querySelectorAll("input[type='search'], [data-garage-search]")].find(isVisible);
      if (search) { event.preventDefault(); search.focus(); search.select(); }
    }
    if (modifier && key === "s") {
      event.preventDefault();
      const dialog = openDialogs().at(-1);
      const panel = document.activeElement?.closest(".career-panel") || document.querySelector(".career-panel.is-active");
      const scope = dialog || panel;
      if (!scope) return;
      const invalid = [...scope.querySelectorAll("input, select, textarea")].find((node) => isVisible(node) && !node.checkValidity());
      if (invalid) { invalid.reportValidity(); return; }
      const explicit = dialog ? [...dialog.querySelectorAll("button.apply, [data-primary-save]")].filter(isVisible)
        : [...scope.querySelectorAll("button[id*='Save'], button[id*='save']")].filter(isVisible);
      // Avoid guessing which of several independent settings forms to submit.
      const nearest = document.activeElement?.closest(".career-card, .career-widget");
      const target = explicit.length === 1 ? explicit[0] : explicit.find((node) => nearest?.contains(node));
      if (target && target.getAttribute("aria-busy") !== "true") target.click();
    }
    if (event.key === "Escape") {
      const dialog = openDialogs().at(-1);
      const close = dialog && [...dialog.querySelectorAll("[data-dialog-close], button[id$='Cancel'], button[id$='Close'], .modal-close")].find(isVisible);
      if (close) { event.preventDefault(); close.click(); }
    }
  }, options);
  const tables = installTables();
  void installStatusBar();
  window.addEventListener("pagehide", () => { clearTimeout(resumeTimer); tables.disconnect(); controller.abort(); }, { once: true });
}

async function installStatusBar() {
  if (!document.querySelector(".workspace-shell, .career-layout")) return;
  const bar = document.createElement("footer");
  bar.className = "desktop-statusbar";
  const context = document.createElement("span");
  const freshness = document.createElement("span");
  freshness.setAttribute("role", "status");
  bar.append(context, freshness);
  document.body.append(bar);
  const [waiting, checked, running, stopped, dirty, busy] = await Promise.all([
    t("save_sync.waiting"), t("desktop.checked_at"), t("desktop.game_running"),
    t("desktop.game_stopped"), t("ui_refinement.unsaved_changes"), t("save_sync.synchronizing"),
  ]);
  const update = () => {
    const game = (window.selectedGame || frontendState.game).toUpperCase();
    const profile = document.getElementById("profileNameDisplay")?.textContent || "";
    const save = document.getElementById("saveName")?.textContent || "";
    const gameRunning = document.getElementById("statusGameRunning")?.classList.contains("is-active");
    context.textContent = [game, gameRunning ? running : stopped, profile, save].filter(Boolean).join(" · ");
    context.title = [frontendState.profilePath, frontendState.savePath].filter(Boolean).join("\n");
    const sync = frontendState.synchronization;
    const time = sync?.lastSuccess || frontendState.lastChecked;
    freshness.textContent = sync?.running ? busy : sync?.pending || document.querySelector('[data-dirty="true"]') ? dirty
      : time ? checked.replaceAll("{time}", new Date(time).toLocaleTimeString(document.documentElement.lang)) : waiting;
  };
  window.addEventListener("desktop-state-changed", update);
  document.addEventListener("savegame-synchronized", update);
  document.addEventListener("input", update);
  document.addEventListener("change", update);
  const status = document.getElementById("statusGameRunning");
  const observer = new MutationObserver(update);
  if (status) observer.observe(status, { attributes: true, attributeFilter: ["class"] });
  window.addEventListener("pagehide", () => observer.disconnect(), { once: true });
  update();
}

function installTables() {
  const bindings = new Map();
  let scheduled = false;
  let disposed = false;
  const enhance = async () => {
    scheduled = false;
    const [searchCopy, countCopy, sortCopy] = await Promise.all([t("desktop.search_table"), t("desktop.row_count"), t("desktop.sort_column")]);
    if (disposed) return;
    for (const [table, binding] of bindings) {
      if (!table.isConnected) { binding.dispose(); bindings.delete(table); }
    }
    for (const table of document.querySelectorAll("table")) {
      if (bindings.has(table) || !table.tHead || !table.tBodies.length || table.closest("[data-no-table-tools]")) continue;
      const scopeKey = `${location.pathname}:${table.id || table.tBodies[0].id || bindings.size}`;
      const toolbar = document.createElement("div");
      toolbar.className = "desktop-table-tools";
      const search = document.createElement("input");
      search.type = "search";
      search.placeholder = searchCopy;
      search.setAttribute("aria-label", searchCopy);
      search.dataset.translatePlaceholder = "desktop.search_table";
      search.dataset.translateAriaLabel = "desktop.search_table";
      const saved = readPreference(`table:${scopeKey}`, {});
      search.value = saved.search || "";
      const count = document.createElement("span");
      count.setAttribute("role", "status");
      toolbar.append(search, count);
      table.before(toolbar);
      let sorting = saved.sorting || null;
      let timer;
      let rows = [];
      const collator = new Intl.Collator(document.documentElement.lang, { numeric: true, sensitivity: "base" });
      const apply = () => {
        const query = search.value.toLocaleLowerCase().trim();
        let shown = 0;
        for (const row of rows) {
          row.node.hidden = Boolean(query && !row.text.includes(query));
          if (!row.node.hidden) shown++;
        }
        if (sorting) {
          const ordered = [...rows].sort((a, b) => collator.compare(a.cells[sorting.index] || "", b.cells[sorting.index] || "") * sorting.direction);
          observer.disconnect();
          const body = table.tBodies[0];
          for (const row of ordered) body.appendChild(row.node);
          observer.observe(table, { childList: true, subtree: true, characterData: true });
        }
        count.textContent = countCopy.replaceAll("{shown}", shown).replaceAll("{total}", rows.length);
        savePreference(`table:${scopeKey}`, { search: search.value, sorting });
      };
      const rebuild = () => {
        rows = [...table.tBodies[0].rows].filter((row) => !row.querySelector("td[colspan]"))
          .map((node) => ({ node, text: node.textContent.toLocaleLowerCase(), cells: [...node.cells].map((cell) => cell.textContent.trim()) }));
        apply();
      };
      const observer = new MutationObserver(() => { clearTimeout(timer); timer = setTimeout(rebuild, 40); });
      observer.observe(table, { childList: true, subtree: true, characterData: true });
      [...table.tHead.rows[0].cells].forEach((cell, index) => {
        if (!cell.textContent.trim() || cell.querySelector("button, input, select")) return;
        const button = document.createElement("button");
        button.type = "button";
        button.className = "desktop-sort";
        button.textContent = cell.textContent;
        button.setAttribute("aria-label", sortCopy.replaceAll("{column}", cell.textContent));
        button.addEventListener("click", () => {
          sorting = { index, direction: sorting?.index === index ? -sorting.direction : 1 };
          [...table.tHead.rows[0].cells].forEach((header, position) => header.setAttribute("aria-sort", position === index ? sorting.direction === 1 ? "ascending" : "descending" : "none"));
          apply();
        });
        cell.replaceChildren(button);
      });
      search.addEventListener("input", () => { clearTimeout(timer); timer = setTimeout(apply, 100); });
      bindings.set(table, { dispose() { observer.disconnect(); clearTimeout(timer); toolbar.remove(); } });
      rebuild();
    }
  };
  const observer = new MutationObserver((mutations) => {
    if (scheduled || !mutations.some((change) => [...change.addedNodes].some((node) => node.nodeType === 1 && (node.matches("table") || node.querySelector("table"))))) return;
    scheduled = true;
    queueMicrotask(enhance);
  });
  observer.observe(document.body, { childList: true, subtree: true });
  void enhance();
  return { disconnect() { disposed = true; observer.disconnect(); for (const binding of bindings.values()) binding.dispose(); bindings.clear(); } };
}

if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", installDesktop, { once: true });
else installDesktop();

export function showDesktopError(key, error) {
  return t(key).then((message) => notify(message, { type: "error", details: error?.stack || error?.message || String(error) }));
}
