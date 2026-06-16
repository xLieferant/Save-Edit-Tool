import { safeInvoke } from "../modules/shared/runtime.js";
import { t } from "../modules/shared/i18n.js";

const tauri = window.__TAURI__;
const updaterApi = tauri?.updater;
const processApi = tauri?.process;
const MEGABYTE = 1024 * 1024;
const PROGRESS_LOG_STEP_PERCENT = 10;
const PROGRESS_LOG_STEP_BYTES = 5 * MEGABYTE;

let currentUpdate = null;
let isUpdateCheckRunning = false;
let isUpdateDownloading = false;
let isUpdateInstalling = false;
let releaseNotesExpanded = false;
let lastLoggedProgressMarker = -1;
let modalRefs = null;
let modalBound = false;
let fallbackModalCreated = false;

function hasUpdaterRuntime() {
  return Boolean(updaterApi?.check && processApi?.relaunch);
}

function injectFallbackModalStyles() {
  if (document.getElementById("fallbackUpdateModalStyles")) return;

  const style = document.createElement("style");
  style.id = "fallbackUpdateModalStyles";
  style.textContent = `
    #modalAppUpdate.updater-fallback-modal {
      position: fixed;
      inset: 0;
      z-index: 10000;
      display: none;
      align-items: center;
      justify-content: center;
      padding: 24px;
      background: rgba(3, 7, 18, 0.72);
      color: #f8fafc;
      font-family: Bahnschrift, "Segoe UI", sans-serif;
    }

    .updater-fallback-modal .modal-box--update {
      width: min(620px, 100%);
      max-height: min(760px, 92vh);
      overflow: auto;
      padding: 22px;
      border: 1px solid rgba(148, 163, 184, 0.28);
      border-radius: 10px;
      background: #111827;
      box-shadow: 0 24px 70px rgba(0, 0, 0, 0.42);
    }

    .updater-fallback-modal .detail-modal-head,
    .updater-fallback-modal .update-progress-head,
    .updater-fallback-modal .modal-actions {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
    }

    .updater-fallback-modal .modal-kicker,
    .updater-fallback-modal .detail-card-label,
    .updater-fallback-modal .update-progress-copy,
    .updater-fallback-modal .update-restart-note {
      color: #94a3b8;
      font-size: 12px;
    }

    .updater-fallback-modal h2 {
      margin: 4px 0 8px;
      font-size: 22px;
    }

    .updater-fallback-modal .modal-description {
      margin: 0;
      color: #cbd5e1;
      line-height: 1.45;
    }

    .updater-fallback-modal .modal-status-pill {
      padding: 6px 10px;
      border-radius: 999px;
      background: rgba(34, 197, 94, 0.16);
      color: #86efac;
      white-space: nowrap;
    }

    .updater-fallback-modal .update-modal-grid {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 10px;
      margin: 18px 0;
    }

    .updater-fallback-modal .detail-card,
    .updater-fallback-modal .update-progress-panel {
      padding: 12px;
      border: 1px solid rgba(148, 163, 184, 0.2);
      border-radius: 8px;
      background: rgba(15, 23, 42, 0.72);
    }

    .updater-fallback-modal .detail-card-value {
      display: block;
      margin-top: 4px;
      overflow-wrap: anywhere;
    }

    .updater-fallback-modal .update-progress-track {
      height: 8px;
      margin-top: 10px;
      overflow: hidden;
      border-radius: 999px;
      background: rgba(51, 65, 85, 0.9);
    }

    .updater-fallback-modal .update-progress-fill {
      width: 0%;
      height: 100%;
      background: #38bdf8;
      transition: width 160ms ease;
    }

    .updater-fallback-modal .update-release-notes-text {
      max-height: 180px;
      overflow: auto;
      white-space: pre-wrap;
      color: #e2e8f0;
    }

    .updater-fallback-modal .modal-actions {
      justify-content: flex-end;
      margin-top: 16px;
    }

    .updater-fallback-modal button {
      padding: 9px 13px;
      border: 1px solid rgba(148, 163, 184, 0.28);
      border-radius: 7px;
      background: rgba(15, 23, 42, 0.92);
      color: #f8fafc;
      cursor: pointer;
    }

    .updater-fallback-modal button.apply {
      border-color: rgba(56, 189, 248, 0.45);
      background: #0284c7;
    }

    .updater-fallback-modal button:disabled {
      cursor: not-allowed;
      opacity: 0.55;
    }

    .updater-fallback-modal .update-modal-note {
      margin-top: 12px;
      color: #fca5a5;
    }
  `;
  document.head.appendChild(style);
}

function createFallbackUpdateModal() {
  if (fallbackModalCreated) return document.getElementById("modalAppUpdate");
  fallbackModalCreated = true;

  if (!document.body) {
    console.warn("[updater] cannot create fallback modal before document.body exists");
    return null;
  }

  injectFallbackModalStyles();

  const modal = document.createElement("div");
  modal.id = "modalAppUpdate";
  modal.className = "modal-backdrop updater-fallback-modal";
  modal.setAttribute("aria-hidden", "true");
  modal.innerHTML = `
    <div class="modal-box modal-box--sheet modal-box--update">
      <div class="detail-modal-head">
        <div>
          <span class="modal-kicker" data-translate="update.window_title"></span>
          <h2 data-translate="update.title"></h2>
          <p class="modal-description" data-translate="update.available"></p>
        </div>
        <div id="modalAppUpdatePhasePill" class="modal-status-pill" data-state="success"></div>
      </div>

      <div class="update-modal-grid">
        <article class="detail-card">
          <span class="detail-card-label" data-translate="update.current_version"></span>
          <strong id="modalAppUpdateCurrentVersion" class="detail-card-value">-</strong>
        </article>
        <article class="detail-card">
          <span class="detail-card-label" data-translate="update.new_version"></span>
          <strong id="modalAppUpdateNewVersion" class="detail-card-value">-</strong>
        </article>
        <article class="detail-card">
          <span class="detail-card-label" data-translate="update.release_date"></span>
          <strong id="modalAppUpdateReleaseDate" class="detail-card-value">-</strong>
        </article>
        <article class="detail-card">
          <span class="detail-card-label" data-translate="update.download_size"></span>
          <strong id="modalAppUpdateDownloadSize" class="detail-card-value">-</strong>
        </article>
      </div>

      <section class="detail-card update-progress-panel">
        <div class="update-progress-head">
          <span class="detail-card-label" data-translate="update.progress"></span>
          <strong id="modalAppUpdateProgressPercent" class="detail-card-value">--</strong>
        </div>
        <p id="modalAppUpdateStatusText" class="modal-description">-</p>
        <div class="update-progress-track" aria-hidden="true">
          <div id="modalAppUpdateProgressFill" class="update-progress-fill"></div>
        </div>
        <p id="modalAppUpdateDownloadedText" class="update-progress-copy">-</p>
        <p id="modalAppUpdateRestartNotice" class="update-restart-note"></p>
      </section>

      <div id="modalAppUpdateNote" class="update-modal-note" data-tone="info" hidden></div>

      <section id="modalAppUpdateReleaseNotesSection" class="detail-card update-release-notes" hidden>
        <div class="update-progress-head">
          <span class="detail-card-label" data-translate="update.release_notes"></span>
        </div>
        <pre id="modalAppUpdateReleaseNotesText" class="update-release-notes-text"></pre>
      </section>

      <div class="modal-actions modal-actions--end">
        <button id="modalAppUpdateLater" type="button" data-translate="update.later"></button>
        <button id="modalAppUpdateViewChangelog" type="button" data-translate="update.view_changelog" hidden></button>
        <button id="modalAppUpdateDownload" class="apply" type="button" data-translate="update.download"></button>
      </div>
    </div>
  `;
  document.body.appendChild(modal);
  void translateFallbackModal(modal);
  return modal;
}

async function translateFallbackModal(root) {
  const nodes = root.querySelectorAll("[data-translate]");
  for (const node of nodes) {
    node.textContent = await t(node.getAttribute("data-translate"));
  }
}

function ensureModalRefs() {
  if (modalRefs) return modalRefs;

  const modal = document.getElementById("modalAppUpdate") || createFallbackUpdateModal();
  if (!modal) {
    console.warn("[updater] update modal is unavailable");
    return null;
  }

  modalRefs = {
    modal,
    phasePill: document.getElementById("modalAppUpdatePhasePill"),
    currentVersion: document.getElementById("modalAppUpdateCurrentVersion"),
    newVersion: document.getElementById("modalAppUpdateNewVersion"),
    releaseDate: document.getElementById("modalAppUpdateReleaseDate"),
    downloadSize: document.getElementById("modalAppUpdateDownloadSize"),
    statusText: document.getElementById("modalAppUpdateStatusText"),
    progressFill: document.getElementById("modalAppUpdateProgressFill"),
    progressPercent: document.getElementById("modalAppUpdateProgressPercent"),
    downloadedText: document.getElementById("modalAppUpdateDownloadedText"),
    note: document.getElementById("modalAppUpdateNote"),
    releaseNotesSection: document.getElementById("modalAppUpdateReleaseNotesSection"),
    releaseNotesText: document.getElementById("modalAppUpdateReleaseNotesText"),
    restartNotice: document.getElementById("modalAppUpdateRestartNotice"),
    laterButton: document.getElementById("modalAppUpdateLater"),
    viewChangelogButton: document.getElementById("modalAppUpdateViewChangelog"),
    downloadButton: document.getElementById("modalAppUpdateDownload"),
  };

  return modalRefs;
}

function isUpdateBusy() {
  return isUpdateDownloading || isUpdateInstalling;
}

async function logUpdateAction(action, stage = "info") {
  await safeInvoke("log_user_action", { action, stage }, { silent: true });
}

function formatBytes(bytes) {
  const numericBytes = Number(bytes ?? 0);
  if (!Number.isFinite(numericBytes) || numericBytes <= 0) return "0 MB";
  if (numericBytes >= MEGABYTE) {
    return `${(numericBytes / MEGABYTE).toFixed(1)} MB`;
  }
  return `${(numericBytes / 1024).toFixed(1)} KB`;
}

function formatUpdateDate(value) {
  if (!value) return "-";
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return String(value);
  return parsed.toLocaleDateString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

function extractUpdateSize(rawJson) {
  if (!rawJson || typeof rawJson !== "object") return 0;

  const stack = [{ value: rawJson, depth: 0 }];
  const visited = new Set();
  const preferredKeys = new Set(["size", "download_size", "downloadSize", "contentLength", "content_length"]);

  while (stack.length > 0) {
    const entry = stack.pop();
    if (!entry || typeof entry.value !== "object" || entry.depth > 5) continue;
    if (visited.has(entry.value)) continue;
    visited.add(entry.value);

    for (const [key, value] of Object.entries(entry.value)) {
      if (preferredKeys.has(key) && Number.isFinite(Number(value)) && Number(value) > 0) {
        return Number(value);
      }
      if (value && typeof value === "object") {
        stack.push({ value, depth: entry.depth + 1 });
      }
    }
  }

  return 0;
}

function getShowToast(showToast) {
  return typeof showToast === "function"
    ? showToast
    : typeof window.showToast === "function"
      ? window.showToast
      : null;
}

async function notify(showToast, key, params = {}, type = "info") {
  const handler = getShowToast(showToast);
  if (handler) {
    await handler(key, params, type);
  }
}

function setModalVisible(visible) {
  const refs = ensureModalRefs();
  if (!refs) return;
  refs.modal.style.display = visible ? "flex" : "none";
  refs.modal.setAttribute("aria-hidden", visible ? "false" : "true");
}

async function setPhase(key, state = "loading", params = {}) {
  const refs = ensureModalRefs();
  if (!refs) return;
  refs.phasePill.dataset.state = state;
  refs.phasePill.textContent = await t(key, params);
}

async function setStatusText(key, params = {}) {
  const refs = ensureModalRefs();
  if (!refs) return;
  refs.statusText.textContent = await t(key, params);
}

async function setStatus(key, state = "loading", params = {}) {
  await setPhase(key, state, params);
  await setStatusText(key, params);
}

async function setProgress(downloadedBytes, totalBytes) {
  const refs = ensureModalRefs();
  if (!refs) return;

  const total = Number(totalBytes ?? 0);
  const downloaded = Math.max(0, Number(downloadedBytes ?? 0));
  const hasKnownTotal = Number.isFinite(total) && total > 0;
  const percent = hasKnownTotal ? Math.max(0, Math.min(100, (downloaded / total) * 100)) : null;

  refs.progressFill.classList.toggle("is-indeterminate", !hasKnownTotal);
  refs.progressFill.style.width = hasKnownTotal ? `${percent}%` : "40%";
  refs.progressPercent.textContent = hasKnownTotal ? `${Math.round(percent)}%` : "--";
  refs.downloadedText.textContent = hasKnownTotal
    ? await t("update.downloaded_of_total", { downloaded: formatBytes(downloaded), total: formatBytes(total) })
    : await t("update.downloaded", { downloaded: formatBytes(downloaded) });
}

async function resetProgressDisplay(totalBytes = 0) {
  const refs = ensureModalRefs();
  if (!refs) return;

  refs.progressFill.classList.remove("is-indeterminate");
  refs.progressFill.style.width = "0%";
  refs.progressPercent.textContent = "0%";
  refs.downloadedText.textContent = totalBytes > 0
    ? await t("update.downloaded_of_total", { downloaded: formatBytes(0), total: formatBytes(totalBytes) })
    : await t("update.downloaded", { downloaded: formatBytes(0) });
}

async function setNote(messageKeyOrText, tone = "info", params = {}) {
  const refs = ensureModalRefs();
  if (!refs?.note) return;
  const isTranslationKey = typeof messageKeyOrText === "string" && messageKeyOrText.includes(".");
  refs.note.hidden = false;
  refs.note.dataset.tone = tone;
  refs.note.textContent = isTranslationKey
    ? await t(messageKeyOrText, params)
    : String(messageKeyOrText ?? "");
}

function clearNote() {
  const refs = ensureModalRefs();
  if (!refs?.note) return;
  refs.note.hidden = true;
  refs.note.textContent = "";
  refs.note.dataset.tone = "info";
}

function updateModalButtons() {
  const refs = ensureModalRefs();
  if (!refs) return;
  const busy = isUpdateBusy();
  refs.downloadButton.disabled = busy || !currentUpdate;
  refs.laterButton.disabled = busy;
  refs.viewChangelogButton.disabled = busy;
}

async function closeCurrentUpdateHandle() {
  if (!currentUpdate) return;
  try {
    await currentUpdate.close();
  } catch (error) {
    console.warn("[updater] update handle close failed", error);
  } finally {
    currentUpdate = null;
  }
}

async function closeUpdateModal() {
  if (isUpdateBusy()) return;
  setModalVisible(false);
  clearNote();
  releaseNotesExpanded = false;
  const refs = ensureModalRefs();
  if (refs?.releaseNotesSection) {
    refs.releaseNotesSection.hidden = true;
  }
  await closeCurrentUpdateHandle();
  updateModalButtons();
}

function bindModal() {
  if (modalBound) return;
  const refs = ensureModalRefs();
  if (!refs) return;

  refs.downloadButton?.addEventListener("click", () => {
    void downloadAndInstallCurrentUpdate();
  });

  refs.laterButton?.addEventListener("click", () => {
    void closeUpdateModal();
  });

  refs.viewChangelogButton?.addEventListener("click", async () => {
    releaseNotesExpanded = !releaseNotesExpanded;
    refs.releaseNotesSection.hidden = !releaseNotesExpanded;
    refs.viewChangelogButton.textContent = await t(
      releaseNotesExpanded ? "update.hide_changelog" : "update.view_changelog"
    );
  });

  refs.modal?.addEventListener("click", (event) => {
    if (event.target === refs.modal && !isUpdateBusy()) {
      void closeUpdateModal();
    }
  });

  modalBound = true;
}

async function renderUpdateModal(update) {
  const refs = ensureModalRefs();
  if (!refs) return;

  const sizeBytes = extractUpdateSize(update?.rawJson);
  const hasReleaseNotes = Boolean(String(update?.body ?? "").trim());

  refs.currentVersion.textContent = String(update?.currentVersion ?? "-");
  refs.newVersion.textContent = String(update?.version ?? "-");
  refs.releaseDate.textContent = formatUpdateDate(update?.date);
  refs.downloadSize.textContent = sizeBytes > 0 ? formatBytes(sizeBytes) : "-";
  refs.releaseNotesText.textContent = hasReleaseNotes
    ? String(update.body)
    : await t("update.release_notes_empty");
  refs.releaseNotesSection.hidden = true;
  refs.viewChangelogButton.hidden = !hasReleaseNotes;
  refs.viewChangelogButton.textContent = await t("update.view_changelog");
  refs.restartNotice.textContent = await t("update.restart_notice");

  clearNote();
  await resetProgressDisplay(sizeBytes);
  await setPhase("update.ready", "success");
  await setStatusText("update.available");
  updateModalButtons();
  setModalVisible(true);
}

function getProgressMarker(downloadedBytes, totalBytes) {
  const total = Number(totalBytes ?? 0);
  const downloaded = Number(downloadedBytes ?? 0);
  if (Number.isFinite(total) && total > 0) {
    const percent = Math.floor((downloaded / total) * 100);
    return Math.floor(percent / PROGRESS_LOG_STEP_PERCENT);
  }
  return Math.floor(downloaded / PROGRESS_LOG_STEP_BYTES);
}

async function maybeLogProgress(downloadedBytes, totalBytes) {
  const marker = getProgressMarker(downloadedBytes, totalBytes);
  if (marker <= lastLoggedProgressMarker) return;
  lastLoggedProgressMarker = marker;
  await logUpdateAction(
    `Update download progress downloaded=${Math.round(downloadedBytes)} total=${Math.round(totalBytes || 0)}`
  );
}

async function checkForUpdate(showToast, options = {}) {
  if (!hasUpdaterRuntime()) {
    console.warn("[updater] skipped because updater runtime is unavailable");
    await notify(showToast, "update.check_failed", {}, "error");
    return null;
  }

  if (isUpdateCheckRunning) {
    console.warn("[updater] skipped because update check is already running");
    return null;
  }

  const { manual = false } = options;
  isUpdateCheckRunning = true;

  try {
    console.log("[updater] checking for updates ...");
    await logUpdateAction("Update check started");
    const update = await updaterApi.check();

    if (!update) {
      await logUpdateAction("No update available");
      if (manual) {
        await notify(showToast, "update.no_update_available", {}, "info");
      }
      return null;
    }

    await logUpdateAction(
      `Update available current=${String(update.currentVersion ?? "-")} latest=${String(update.version ?? "-")}`
    );

    if (currentUpdate && currentUpdate !== update && !isUpdateBusy()) {
      await closeCurrentUpdateHandle();
    }

    currentUpdate = update;
    bindModal();
    await renderUpdateModal(update);
    return update;
  } catch (error) {
    console.warn("[updater] check failed but app continues", error);
    await logUpdateAction(`Update check failed: ${String(error?.message || error)}`, "error");
    await notify(showToast, "update.check_failed", {}, "error");
    return null;
  } finally {
    isUpdateCheckRunning = false;
  }
}

async function downloadAndInstallCurrentUpdate(showToast) {
  if (!currentUpdate || isUpdateDownloading) return;

  const refs = ensureModalRefs();
  if (!refs) return;

  let downloadedBytes = 0;
  let totalBytes = 0;
  isUpdateDownloading = true;
  lastLoggedProgressMarker = -1;
  updateModalButtons();

  try {
    await setStatus("update.preparing", "loading");
    await logUpdateAction("Update download started");

    await currentUpdate.download((event) => {
      if (!event || typeof event !== "object") return;

      switch (event.event) {
        case "Started":
          totalBytes = Number(event.data?.contentLength ?? 0);
          downloadedBytes = 0;
          void setStatus("update.downloading", "loading");
          void setProgress(downloadedBytes, totalBytes);
          void maybeLogProgress(downloadedBytes, totalBytes);
          break;
        case "Progress":
          downloadedBytes += Number(event.data?.chunkLength ?? 0);
          void setProgress(downloadedBytes, totalBytes);
          void maybeLogProgress(downloadedBytes, totalBytes);
          break;
        case "Finished":
          if (totalBytes <= 0) {
            totalBytes = downloadedBytes;
          }
          break;
        default:
          break;
      }
    });

    await setProgress(totalBytes || downloadedBytes, totalBytes || downloadedBytes);
    await setStatus("update.download_finished", "success");
    await logUpdateAction("Update download finished");
  } catch (error) {
    console.error("[updater] download failed", error);
    await logUpdateAction(`Update download failed: ${String(error?.message || error)}`, "error");
    await setStatus("update.download_failed", "error");
    await setNote("update.download_failed", "error");
    await notify(showToast, "update.download_failed", {}, "error");
    isUpdateDownloading = false;
    updateModalButtons();
    return;
  }

  isUpdateDownloading = false;
  isUpdateInstalling = true;
  updateModalButtons();

  try {
    await setStatus("update.installing", "loading");
    await logUpdateAction("Update install started");
    await currentUpdate.install();

    await setStatus("update.restarting", "warning");
    await setNote("update.restart_notice", "warning");
    await logUpdateAction("Update relaunch started");
    await processApi.relaunch();
  } catch (error) {
    console.error("[updater] install failed", error);
    await logUpdateAction(`Update install failed: ${String(error?.message || error)}`, "error");
    await setStatus("update.install_failed", "error");
    await setNote("update.install_failed", "error");
    await notify(showToast, "update.install_failed", {}, "error");
  } finally {
    isUpdateInstalling = false;
    updateModalButtons();
  }
}

export async function checkUpdaterOnStartup(showToast) {
  try {
    await checkForUpdate(showToast, { manual: false });
  } catch (error) {
    console.warn("[updater] check failed but app continues", error);
  }
}

export async function manualUpdateCheck(showToast) {
  try {
    await checkForUpdate(showToast, { manual: true });
  } catch (error) {
    console.warn("[updater] manual check failed but app continues", error);
  }
}
