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

function hasUpdaterRuntime() {
  return Boolean(updaterApi?.check && processApi?.relaunch);
}

function ensureModalRefs() {
  if (modalRefs) return modalRefs;

  const modal = document.getElementById("modalAppUpdate");
  if (!modal) return null;

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
    await notify(showToast, "update.check_failed", {}, "error");
    return null;
  }

  if (isUpdateCheckRunning) {
    return null;
  }

  const { manual = false } = options;
  isUpdateCheckRunning = true;

  try {
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
    console.error("[updater] check failed", error);
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
  await checkForUpdate(showToast, { manual: false });
}

export async function manualUpdateCheck(showToast) {
  await checkForUpdate(showToast, { manual: true });
}
