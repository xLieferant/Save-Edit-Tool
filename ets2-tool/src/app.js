import { tools } from "./tools.js";
import {
  clampLevel,
  clampXp,
  getLevelForXp,
  getMaxLevel,
  getXpForLevel,
  loadLevelTable,
} from "./js/level-system.js";
import { mountSkilltreeEditor } from "./js/skilltree.js";

/* --------------------------------------------------------------
   TOOL LOADER UND TAB HANDLING
-------------------------------------------------------------- */
const container = document.querySelector("#tool-container");
const navButtons = document.querySelectorAll(".editor-tabs .nav-btn");
const skilltreeModal = document.getElementById("modalSkilltree");
const skilltreeModalRoot = document.getElementById("skilltreeModalRoot");
const skilltreeModalClose = document.getElementById("modalSkilltreeClose");
export let activeTab = "profile";
let loadToolsRenderId = 0;
const editorTabShortcuts = {
  F1: "truck",
  F2: "trailer",
  F3: "profile",
  F4: "settings",
};

function setActiveEditorTabButton(tab) {
  const nextButton = document.querySelector(`.editor-tabs .nav-btn[data-tab="${tab}"]`);
  if (!nextButton) return null;

  document.querySelector(".editor-tabs .nav-btn.active")?.classList.remove("active");
  nextButton.classList.add("active");
  return nextButton;
}

async function activateEditorTab(tab) {
  const nextButton = setActiveEditorTabButton(tab);
  if (!nextButton) return;
  await loadTools(nextButton.dataset.tab);
}

export async function loadTools(tab) {
  console.log(`[app.js] Lade Tools für Tab: ${tab}`);

  activeTab = tab;
  if (!container) return;
  container.innerHTML = "";
  const renderId = ++loadToolsRenderId;
  const toolList = tools[tab] || [];
  const hasSkilltreeLauncher = tab === "profile";
  const tabLabelKeyMap = {
    truck: "editor.tab.truck",
    trailer: "editor.tab.trailer",
    profile: "editor.tab.profile",
    settings: "editor.tab.settings",
  };

  document.dispatchEvent(new CustomEvent("editor-tab-changed", { detail: { tab } }));

  if (hasSkilltreeLauncher) {
    const launcherCard = await createSkilltreeLauncherCard();
    if (renderId !== loadToolsRenderId) return;
    container.appendChild(launcherCard);
  }

  if (!toolList.length && !hasSkilltreeLauncher) {
    container.innerHTML = `
      <article class="tool-card">
        <div class="tool-content">
          <span class="overview-label">${await window.t("editor.stage_label")}</span>
          <h3>${await window.t("editor.empty_title")}</h3>
          <p>${await window.t("editor.empty_desc")}</p>
        </div>
      </article>
    `;
    return;
  }

  for (const t of toolList) {
    if (t.hidden) continue; // unsichtbare Tools überspringen

    const card = document.createElement("div");
    card.classList.add("tool-card");

    const title = await window.t(t.title);
    const desc = await window.t(t.desc);
    const open = await window.t("modals.open");
    const category = await window.t(tabLabelKeyMap[tab] || "editor.tab.profile");
    if (renderId !== loadToolsRenderId) return;

    card.innerHTML = `
      <img src="${t.img}" alt="${title}">
      <div class="tool-content">
          <span class="overview-label">${category}</span>
          <h3>${title}</h3>
          <p>${desc}</p>
          <button>${open}</button>
      </div>
    `;

    const btn = card.querySelector("button");

    if (t.disabled) {
      btn.disabled = true;
      btn.classList.add("modal-disabled"); // CSS: rot + cursornot-allowed
      btn.textContent = await window.t("toasts.coming_soon");
    } else {
      btn.addEventListener("click", t.action);
    }

    container.appendChild(card);
  }
}

async function createSkilltreeLauncherCard() {
  const card = document.createElement("div");
  card.className = "tool-card skilltree-launch-card";

  const title = await window.t("editor.skilltree.title");
  const desc = await window.t("editor.skilltree.launch_desc");
  const kicker = await window.t("editor.skilltree.launch_kicker");
  const open = await window.t("editor.skilltree.launch_action");

  card.innerHTML = `
    <div class="skilltree-launch-visual" aria-hidden="true">
      <span></span>
      <span></span>
      <span></span>
    </div>
    <div class="tool-content">
        <span class="overview-label">${kicker}</span>
        <h3>${title}</h3>
        <p>${desc}</p>
        <button type="button">${open}</button>
    </div>
  `;

  card.querySelector("button")?.addEventListener("click", openSkilltreeModal);
  return card;
}

async function openSkilltreeModal() {
  if (!skilltreeModal || !skilltreeModalRoot) return;

  skilltreeModal.style.display = "flex";
  await mountSkilltreeEditor(skilltreeModalRoot);
  skilltreeModalClose?.focus();
}

function closeSkilltreeModal() {
  if (!skilltreeModal) return;
  skilltreeModal.style.display = "none";
}

skilltreeModalClose?.addEventListener("click", closeSkilltreeModal);
skilltreeModal?.addEventListener("click", (event) => {
  if (event.target === skilltreeModal) {
    closeSkilltreeModal();
  }
});

navButtons.forEach((btn) => {
  btn.addEventListener("click", () => {
    void activateEditorTab(btn.dataset.tab);
  });
});

// Default Tab
const defaultTabBtn = document.querySelector(".editor-tabs .nav-btn.active");
if (defaultTabBtn) {
  if (typeof window.t === "function") {
    void activateEditorTab(defaultTabBtn.dataset.tab);
  } else {
    window.addEventListener(
      "translations-ready",
      () => {
        void activateEditorTab(defaultTabBtn.dataset.tab);
      },
      { once: true }
    );
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const savedTheme = localStorage.getItem("theme") || "neon-red";
  document.body.classList.remove("theme-dark", "theme-light", "theme-neon", "theme-neon-red");
  document.body.classList.add(`theme-${savedTheme}`);
});


/* --------------------------------------------------------------
   MODAL REFERENCES
-------------------------------------------------------------- */
const modalText = document.querySelector("#modalText");
const modalTextTitle = document.querySelector("#modalTextTitle");
const modalTextInput = document.querySelector("#modalTextInput");
const modalTextApply = document.getElementById("modalTextApply");
const modalTextCancel = document.getElementById("modalTextCancel");
const modalWiki = document.getElementById("modalWiki");
const modalWikiTitle = document.getElementById("modalWikiTitle");
const modalWikiClose = document.getElementById("modalWikiClose");
const modalWikiCloseTop = document.getElementById("modalWikiCloseTop");
const wikiHelpBtn = document.getElementById("wikiHelpBtn");
const wikiIndexNav = document.getElementById("wikiIndexNav");

const modalNumber = document.querySelector("#modalNumber");
const modalNumberTitle = document.querySelector("#modalNumberTitle");
const modalNumberInput = document.querySelector("#modalNumberInput");
const modalNumberApply = document.getElementById("modalNumberApply");
const modalNumberCancel = document.getElementById("modalNumberCancel");

const modalSlider = document.querySelector("#modalSlider");
const modalSliderTitle = document.querySelector("#modalSliderTitle");
const modalSliderInput = document.querySelector("#modalSliderInput");
const modalSliderApply = document.getElementById("modalSliderApply");
const modalSliderCancel = document.getElementById("modalSliderCancel");

const modalMulti = document.querySelector("#modalMulti");
const modalMultiTitle = document.querySelector("#modalMultiTitle");
const modalMultiContent = document.querySelector("#modalMultiContent");
const modalMultiApplyBtn = document.getElementById("modalMultiApply");
const modalMultiCancelBtn = document.getElementById("modalMultiCancel");

const modalClone = document.getElementById("modalClone");
const cloneSourceDisplay = document.getElementById("cloneSourceDisplay");
const cloneNameInput = document.getElementById("cloneNameInput");
const cloneValidationMsg = document.getElementById("cloneValidationMsg");
const cloneBackup = document.getElementById("cloneBackup");
const modalCloneApply = document.getElementById("modalCloneApply");
const modalCloneCancel = document.getElementById("modalCloneCancel");

const modalTruckInfo = document.getElementById("modalTruckInfo");
const modalTruckInfoState = document.getElementById("modalTruckInfoState");
const modalTruckInfoLoading = document.getElementById("modalTruckInfoLoading");
const modalTruckInfoError = document.getElementById("modalTruckInfoError");
const modalTruckInfoErrorText = document.getElementById("modalTruckInfoErrorText");
const modalTruckInfoEmpty = document.getElementById("modalTruckInfoEmpty");
const modalTruckInfoContent = document.getElementById("modalTruckInfoContent");
const modalTruckInfoName = document.getElementById("modalTruckInfoName");
const modalTruckInfoBrand = document.getElementById("modalTruckInfoBrand");
const modalTruckInfoModel = document.getElementById("modalTruckInfoModel");
const modalTruckInfoOdometer = document.getElementById("modalTruckInfoOdometer");
const modalTruckInfoClose = document.getElementById("modalTruckInfoClose");
const modalLevelSystem = document.getElementById("modalLevelSystem");
const modalLevelSystemModePill = document.getElementById("modalLevelSystemModePill");
const levelSystemCurrentStatus = document.getElementById("levelSystemCurrentStatus");
const levelSystemCurrentBadge = document.getElementById("levelSystemCurrentBadge");
const levelSystemCurrentXp = document.getElementById("levelSystemCurrentXp");
const levelSystemTargetStatus = document.getElementById("levelSystemTargetStatus");
const levelSystemTargetBadge = document.getElementById("levelSystemTargetBadge");
const levelSystemTargetXp = document.getElementById("levelSystemTargetXp");
const levelSystemModeLevel = document.getElementById("levelSystemModeLevel");
const levelSystemModeXp = document.getElementById("levelSystemModeXp");
const levelSystemLevelField = document.getElementById("levelSystemLevelField");
const levelSystemXpField = document.getElementById("levelSystemXpField");
const levelSystemLevelInput = document.getElementById("levelSystemLevelInput");
const levelSystemXpInput = document.getElementById("levelSystemXpInput");
const levelSystemHint = document.getElementById("levelSystemHint");
const levelSystemTargetLevelMeta = document.getElementById("levelSystemTargetLevelMeta");
const levelSystemTargetXpMeta = document.getElementById("levelSystemTargetXpMeta");
const modalLevelSystemApply = document.getElementById("modalLevelSystemApply");
const modalLevelSystemClose = document.getElementById("modalLevelSystemClose");
const modalConflictDiagnostics = document.getElementById("modalConflictDiagnostics");
const modalConflictDiagnosticsConfidence = document.getElementById("modalConflictDiagnosticsConfidence");
const modalConflictDiagnosticsHealth = document.getElementById("modalConflictDiagnosticsHealth");
const modalConflictDiagnosticsLoading = document.getElementById("modalConflictDiagnosticsLoading");
const modalConflictDiagnosticsError = document.getElementById("modalConflictDiagnosticsError");
const modalConflictDiagnosticsErrorText = document.getElementById("modalConflictDiagnosticsErrorText");
const modalConflictDiagnosticsEmpty = document.getElementById("modalConflictDiagnosticsEmpty");
const modalConflictDiagnosticsContent = document.getElementById("modalConflictDiagnosticsContent");
const modalConflictDiagnosticsHeadline = document.getElementById("modalConflictDiagnosticsHeadline");
const modalConflictDiagnosticsSummary = document.getElementById("modalConflictDiagnosticsSummary");
const modalConflictDiagnosticsGuidance = document.getElementById("modalConflictDiagnosticsGuidance");
const diagnosticsRefreshBtn = document.getElementById("diagnosticsRefreshBtn");
const diagnosticsDeepScanBtn = document.getElementById("diagnosticsDeepScanBtn");
const diagnosticsRefreshFooterBtn = document.getElementById("diagnosticsRefreshFooterBtn");
const diagnosticsDeepScanFooterBtn = document.getElementById("diagnosticsDeepScanFooterBtn");
const diagnosticsExportReportBtn = document.getElementById("diagnosticsExportReportBtn");
const diagnosticsSourcesGrid = document.getElementById("diagnosticsSourcesGrid");
const diagnosticsContextGrid = document.getElementById("diagnosticsContextGrid");
const diagnosticsCrashPrimary = document.getElementById("diagnosticsCrashPrimary");
const diagnosticsCrashSummary = document.getElementById("diagnosticsCrashSummary");
const diagnosticsCrashCounts = document.getElementById("diagnosticsCrashCounts");
const diagnosticsCrashContext = document.getElementById("diagnosticsCrashContext");
const diagnosticsSuspectedState = document.getElementById("diagnosticsSuspectedState");
const diagnosticsSuspectedMods = document.getElementById("diagnosticsSuspectedMods");
const diagnosticsMissingReferences = document.getElementById("diagnosticsMissingReferences")
  || document.getElementById("diagnosticsBrokenReferences");
const diagnosticsSeverityFilter = document.getElementById("diagnosticsSeverityFilter");
const diagnosticsErrorsList = document.getElementById("diagnosticsErrorsList");
const diagnosticsLogsInfo = document.getElementById("diagnosticsLogsInfo");
const diagnosticsLimitations = document.getElementById("diagnosticsLimitations");
const diagnosticsExportErrorsBtn = document.getElementById("diagnosticsExportErrorsBtn");
const diagnosticsExportCrashBtn = document.getElementById("diagnosticsExportCrashBtn");
const diagnosticsCopySummaryBtn = document.getElementById("diagnosticsCopySummaryBtn");
const diagnosticsOpenLogFolderBtn = document.getElementById("diagnosticsOpenLogFolderBtn");
const modalConflictDiagnosticsClose = document.getElementById("modalConflictDiagnosticsClose");
const modalConflictDiagnosticsRetryBtn = document.getElementById("modalConflictDiagnosticsRetryBtn");
const modalConflictDiagnosticsDeepBtn = document.getElementById("modalConflictDiagnosticsDeepBtn");
const modalRecoveryCenter = document.getElementById("modalRecoveryCenter");
const modalRecoveryCenterClose = document.getElementById("modalRecoveryCenterClose");
const modalRestorePreview = document.getElementById("modalRestorePreview");
const modalRestorePreviewClose = document.getElementById("modalRestorePreviewClose");
const modalSafeValueReset = document.getElementById("modalSafeValueReset");
const modalSafeValueResetClose = document.getElementById("modalSafeValueResetClose");
const modalUserLogs = document.getElementById("modalUserLogs");
const modalUserLogsClose = document.getElementById("modalUserLogsClose");
const modalModProfileManager = document.getElementById("modalModProfileManager");
const modalModProfileManagerClose = document.getElementById("modalModProfileManagerClose");
const modProfileManagerStatusPill = document.getElementById("modProfileManagerStatusPill");
const modProfileManagerProfilePill = document.getElementById("modProfileManagerProfilePill");
const modSandboxReloadBtn = document.getElementById("modSandboxReloadBtn");
const modSteamConsoleBtn = document.getElementById("modSteamConsoleBtn");
const modSandboxCount = document.getElementById("modSandboxCount");
const modSandboxPresetList = document.getElementById("modSandboxPresetList");
const modSandboxEmpty = document.getElementById("modSandboxEmpty");
const modActiveProfileName = document.getElementById("modActiveProfileName");
const modActiveSaveName = document.getElementById("modActiveSaveName");
const modSandboxProgressList = document.getElementById("modSandboxProgressList");
const modApplySandboxResult = document.getElementById("modApplySandboxResult");

const modalProfileShare = document.getElementById("modalProfileShare");
const profileShareModeKicker = document.getElementById("profileShareModeKicker");
const profileShareModalTitle = document.getElementById("profileShareModalTitle");
const profileShareModalDescription = document.getElementById("profileShareModalDescription");
const profileShareExperimentBadge = document.getElementById("profileShareExperimentBadge");
const profileShareStatusPill = document.getElementById("profileShareStatusPill");
const profileShareStatusTitle = document.getElementById("profileShareStatusTitle");
const profileShareStatusMessage = document.getElementById("profileShareStatusMessage");
const profileShareLastPath = document.getElementById("profileShareLastPath");
const profileShareSummaryLabel = document.getElementById("profileShareSummaryLabel");
const profileShareSummaryTitle = document.getElementById("profileShareSummaryTitle");
const profileShareModeHint = document.getElementById("profileShareModeHint");
const profileShareSourceName = document.getElementById("profileShareSourceName");
const profileShareArchiveName = document.getElementById("profileShareArchiveName");
const profileShareTargetLabel = document.getElementById("profileShareTargetLabel");
const profileShareTargetPath = document.getElementById("profileShareTargetPath");
const profileShareSelectedPathLabel = document.getElementById("profileShareSelectedPathLabel");
const profileShareSelectedPath = document.getElementById("profileShareSelectedPath");
const profileShareWorkspaceTitle = document.getElementById("profileShareWorkspaceTitle");
const profileShareExperimentalCopy = document.getElementById("profileShareExperimentalCopy");
const profileSharePickerLabel = document.getElementById("profileSharePickerLabel");
const profileShareSelectionDisplay = document.getElementById("profileShareSelectionDisplay");
const profileShareBrowseButton = document.getElementById("profileShareBrowseButton");
const profileShareImportOptions = document.getElementById("profileShareImportOptions");
const profileShareImportName = document.getElementById("profileShareImportName");
const profileSharePreview = document.getElementById("profileSharePreview");
const modalProfileSharePrimary = document.getElementById("modalProfileSharePrimary");
const modalProfileShareClose = document.getElementById("modalProfileShareClose");
const saveImportSavesBtn = document.getElementById("saveImportSavesBtn");
const saveExportSavesBtn = document.getElementById("saveExportSavesBtn");
const editorModalDescriptors = [
  { element: modalText, closeButton: modalTextCancel },
  { element: modalWiki, closeButton: modalWikiClose },
  { element: modalNumber, closeButton: modalNumberCancel },
  { element: modalSlider, closeButton: modalSliderCancel },
  { element: modalMulti, closeButton: modalMultiCancelBtn },
  { element: modalClone, closeButton: modalCloneCancel },
  { element: modalTruckInfo, closeButton: modalTruckInfoClose },
  { element: modalLevelSystem, closeButton: modalLevelSystemClose },
  { element: modalConflictDiagnostics, closeButton: modalConflictDiagnosticsClose },
  { element: modalRecoveryCenter, closeButton: modalRecoveryCenterClose },
  { element: modalRestorePreview, closeButton: modalRestorePreviewClose },
  { element: modalSafeValueReset, closeButton: modalSafeValueResetClose },
  { element: modalUserLogs, closeButton: modalUserLogsClose },
  { element: modalModProfileManager, closeButton: modalModProfileManagerClose },
  { element: modalProfileShare, closeButton: modalProfileShareClose },
  { element: skilltreeModal, closeButton: skilltreeModalClose },
];

const WIKI_FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "textarea:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

let lastWikiTrigger = null;

function isEditorModalOpen(element) {
  if (!element || element.hidden) return false;
  return window.getComputedStyle(element).display !== "none";
}

function closeActiveEditorModal() {
  const activeModal = [...editorModalDescriptors]
    .reverse()
    .find(({ element }) => isEditorModalOpen(element));

  if (!activeModal?.closeButton || activeModal.closeButton.disabled) {
    return false;
  }

  activeModal.closeButton.click();
  return true;
}

function handleEditorShortcut(event) {
  if (event.defaultPrevented) return;
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;

  if (event.key === "Escape") {
    if (!closeActiveEditorModal()) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    return;
  }

  const targetTab = editorTabShortcuts[event.key];
  if (!targetTab || event.repeat) return;

  event.preventDefault();
  event.stopImmediatePropagation();
  void activateEditorTab(targetTab);
}

function registerEditorShortcuts() {
  if (window.__ets2_editor_shortcuts_registered) return;
  window.__ets2_editor_shortcuts_registered = true;
  document.addEventListener("keydown", handleEditorShortcut);
}

registerEditorShortcuts();

function getWikiFocusableElements() {
  if (!modalWiki) return [];
  return [...modalWiki.querySelectorAll(WIKI_FOCUSABLE_SELECTOR)].filter((element) => {
    return !element.hasAttribute("hidden") && window.getComputedStyle(element).display !== "none";
  });
}

function handleWikiFocusTrap(event) {
  if (event.key !== "Tab" || !isEditorModalOpen(modalWiki)) return;

  const focusable = getWikiFocusableElements();
  if (!focusable.length) {
    event.preventDefault();
    modalWikiTitle?.focus();
    return;
  }

  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  const active = document.activeElement;

  if (event.shiftKey && active === first) {
    event.preventDefault();
    last.focus();
    return;
  }

  if (!event.shiftKey && active === last) {
    event.preventDefault();
    first.focus();
  }
}

async function syncWikiAccessibilityCopy() {
  if (typeof window.t !== "function") return;

  const helpLabel = await window.t("modals.wiki.button_aria_label");
  const closeLabel = await window.t("modals.wiki.close_aria_label");
  const navLabel = await window.t("modals.wiki.nav_aria_label");

  wikiHelpBtn?.setAttribute("aria-label", helpLabel);
  modalWikiCloseTop?.setAttribute("aria-label", closeLabel);
  wikiIndexNav?.setAttribute("aria-label", navLabel);
}

if (typeof window.t === "function") {
  void syncWikiAccessibilityCopy();
} else {
  window.addEventListener(
    "translations-ready",
    () => {
      void syncWikiAccessibilityCopy();
    },
    { once: true }
  );
}

function closeWikiModal() {
  if (!modalWiki) return;
  document.removeEventListener("keydown", handleWikiFocusTrap);
  modalWiki.style.display = "none";

  const restoreTarget = lastWikiTrigger && document.contains(lastWikiTrigger)
    ? lastWikiTrigger
    : wikiHelpBtn;
  lastWikiTrigger = null;
  restoreTarget?.focus();
}

export async function openWikiModal() {
  if (!modalWiki) return;
  lastWikiTrigger = document.activeElement instanceof HTMLElement ? document.activeElement : wikiHelpBtn;
  modalWiki.style.display = "flex";
  document.addEventListener("keydown", handleWikiFocusTrap);

  window.requestAnimationFrame(() => {
    modalWikiTitle?.focus();
  });
}

if (wikiHelpBtn) {
  wikiHelpBtn.addEventListener("click", () => {
    void openWikiModal();
  });
}

modalWikiClose?.addEventListener("click", closeWikiModal);
modalWikiCloseTop?.addEventListener("click", closeWikiModal);
modalWiki?.addEventListener("click", (event) => {
  if (event.target === modalWiki) {
    closeWikiModal();
  }
});
wikiIndexNav?.addEventListener("click", (event) => {
  const targetLink = event.target.closest("a[href^='#wikiSection']");
  if (!targetLink) return;

  const targetSection = document.querySelector(targetLink.getAttribute("href"));
  if (!targetSection) return;

  event.preventDefault();
  targetSection.scrollIntoView({ behavior: "smooth", block: "start" });
});

function formatMetric(value, digits = 0) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return "-";
  return numeric.toLocaleString(undefined, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
}

function formatDistance(value) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return "-";
  return `${formatMetric(numeric, numeric % 1 === 0 ? 0 : 1)} km`;
}

function formatLevelBadge(level) {
  return `L${formatMetric(level, 0)}`;
}

function safeValue(value, fallback = "-") {
  if (value === null || value === undefined) return fallback;
  const text = String(value).trim();
  return text ? text : fallback;
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\"", "&quot;")
    .replaceAll("'", "&#39;");
}

function setModalPillState(element, state, text) {
  if (!element) return;
  element.dataset.state = state;
  element.textContent = text;
}

function resetTruckModalPanels() {
  modalTruckInfoLoading.hidden = true;
  modalTruckInfoError.hidden = true;
  modalTruckInfoEmpty.hidden = true;
  modalTruckInfoContent.hidden = true;
}

let currentDiagnosticsReport = null;
let currentDiagnosticsSessionId = 0;
let currentDiagnosticsSeverityValue = "all";
let isModAnalysisRunning = false;
const MAX_RENDERED_DIAGNOSTICS_ITEMS = 50;

function resetDiagnosticsModalPanels() {
  if (modalConflictDiagnosticsLoading) modalConflictDiagnosticsLoading.hidden = true;
  if (modalConflictDiagnosticsError) modalConflictDiagnosticsError.hidden = true;
  if (modalConflictDiagnosticsEmpty) modalConflictDiagnosticsEmpty.hidden = true;
  if (modalConflictDiagnosticsContent) modalConflictDiagnosticsContent.hidden = true;
}

function setLevelSystemHint(message = "") {
  if (!levelSystemHint) return;
  levelSystemHint.textContent = message;
  levelSystemHint.hidden = !message;
}

function setDiagnosticsActionState(disabled) {
  if (diagnosticsRefreshBtn) diagnosticsRefreshBtn.disabled = disabled;
  if (diagnosticsDeepScanBtn) diagnosticsDeepScanBtn.disabled = disabled;
  if (diagnosticsRefreshFooterBtn) diagnosticsRefreshFooterBtn.disabled = disabled;
  if (diagnosticsDeepScanFooterBtn) diagnosticsDeepScanFooterBtn.disabled = disabled;
  if (diagnosticsExportReportBtn) diagnosticsExportReportBtn.disabled = disabled;
  if (diagnosticsExportErrorsBtn) diagnosticsExportErrorsBtn.disabled = disabled;
  if (diagnosticsExportCrashBtn) diagnosticsExportCrashBtn.disabled = disabled;
  if (diagnosticsCopySummaryBtn) diagnosticsCopySummaryBtn.disabled = disabled;
  if (diagnosticsOpenLogFolderBtn) diagnosticsOpenLogFolderBtn.disabled = disabled;
  if (diagnosticsSeverityFilter) diagnosticsSeverityFilter.disabled = disabled;
  if (modalConflictDiagnosticsRetryBtn) modalConflictDiagnosticsRetryBtn.disabled = disabled;
  if (modalConflictDiagnosticsDeepBtn) modalConflictDiagnosticsDeepBtn.disabled = disabled;
}

function diagnosticsStatusState(value) {
  if (value === "Clean") return "success";
  if (value === "Warnings") return "warning";
  if (value === "Issues found") return "error";
  return "neutral";
}

function diagnosticsConfidenceState(value) {
  if (value === "High") return "error";
  if (value === "Likely") return "warning";
  if (value === "Possible") return "warning";
  return "neutral";
}

function diagnosticsBadgeTone(value) {
  const normalized = String(value ?? "").toLowerCase();
  if (normalized.includes("clean") || normalized.includes("found") || normalized.includes("active")) {
    return "success";
  }
  if (
    normalized.includes("critical")
    || normalized.includes("error")
    || normalized.includes("high")
    || normalized.includes("issues found")
    || normalized.includes("missing")
    || normalized.includes("not active")
  ) {
    return "danger";
  }
  if (normalized.includes("warning") || normalized.includes("likely") || normalized.includes("possible")) {
    return "warning";
  }
  return "neutral";
}

function createDiagnosticsCopyMap(values) {
  return {
    ...values,
    status: {
      Clean: values.statusClean,
      Warnings: values.statusWarnings,
      "Issues found": values.statusIssuesFound,
      "Not enough data": values.statusNotEnoughData,
    },
    confidence: {
      Low: values.confidenceLow,
      Possible: values.confidencePossible,
      Likely: values.confidenceLikely,
      High: values.confidenceHigh,
    },
    activeState: {
      Active: values.activeStateActive,
      "Not active": values.activeStateNotActive,
      Unknown: values.activeStateUnknown,
    },
    availability: {
      found: values.availabilityFound,
      missing: values.availabilityMissing,
    },
    severity: {
      Info: values.severityInfo,
      Warning: values.severityWarning,
      Error: values.severityError,
      Critical: values.severityCritical,
    },
  };
}

function createDefaultDiagnosticsCopy() {
  return createDiagnosticsCopyMap({
    statusClean: "Clean",
    statusWarnings: "Warnings",
    statusIssuesFound: "Issues found",
    statusNotEnoughData: "Not enough data",
    confidenceLow: "Low",
    confidencePossible: "Possible",
    confidenceLikely: "Likely",
    confidenceHigh: "High",
    activeStateActive: "Active",
    activeStateNotActive: "Not active",
    activeStateUnknown: "Unknown",
    availabilityFound: "Found",
    availabilityMissing: "Missing",
    severityInfo: "Info",
    severityWarning: "Warning",
    severityError: "Error",
    severityCritical: "Critical",
    contextGame: "Game",
    contextProfile: "Profile",
    contextSave: "Save",
    contextBasePath: "Base path",
    sourceGameLog: "game.log.txt",
    sourceGameCrash: "game.crash.txt",
    sourceModFolder: "Mod folder",
    sourceIndexedMods: "Indexed mods",
    sourceExtractedErrors: "Extracted errors",
    sourceActiveMods: "Active mods",
    fieldPackage: "Package",
    fieldFile: "File",
    fieldSource: "Source",
    fieldPath: "Path",
    fieldReadable: "Readable",
    fieldManifest: "Manifest",
    fieldCategories: "Categories",
    fieldProfileInferred: "Profile inferred",
    fieldSaveInferred: "Save inferred",
    noSuspectedMods: "No suspicious mods detected.",
    noMissingReferences: "No missing or removed references detected.",
    noErrors: "No relevant errors were extracted.",
    noCrashContext: "No crash context lines were captured.",
    noLogs: "No analyzer log paths available.",
    noLimitations: "No limitations reported.",
    removedSuspected: "Potential removed mod or stale save reference.",
    noModAssigned: "No mod could be confidently assigned.",
    filterAll: "All severities",
    filterInfo: "Info",
    filterWarning: "Warning",
    filterError: "Error",
    filterCritical: "Critical",
    rawLineLabel: "Raw line",
    lastContextLabel: "Last context",
    logsTechnical: "Technical log",
    logsUser: "User log",
    logsFolder: "Log folder",
    limitLabel: "Limitation",
    protectedModsTitle: "Protected or unreadable mods detected",
    protectedModsHint: "This is common for ETS2 .scs mods and does not necessarily mean the mod is broken.",
    analysisTimedOut: "Mod analysis timed out. Some files were skipped.",
    deepScanWarning: "Deep scan may take longer and some protected mods cannot be inspected.",
    errorBody: "Analysis failed.",
  });
}

async function logDiagnosticsFrontendEvent(event, detail = "", userVisible = false) {
  if (typeof window.invoke !== "function") return;

  try {
    await window.invoke("log_diagnostics_event", {
      event,
      detail: detail || null,
      user_visible: userVisible,
    });
  } catch (error) {
    console.warn("Diagnostics event log failed:", error);
  }
}

function normalizeDiagnosticsItemArray(value, arrayKeys = []) {
  if (!Array.isArray(value)) return [];
  return value.map((item) => {
    const normalized = item && typeof item === "object" ? { ...item } : {};
    for (const key of arrayKeys) {
      normalized[key] = Array.isArray(normalized[key]) ? normalized[key] : [];
    }
    return normalized;
  });
}

function normalizeDiagnosticsReport(report) {
  const normalized = report && typeof report === "object" ? { ...report } : {};
  const crashSummary = normalized.crash_summary && typeof normalized.crash_summary === "object"
    ? { ...normalized.crash_summary }
    : {};

  return {
    ...normalized,
    context: normalized.context && typeof normalized.context === "object" ? { ...normalized.context } : {},
    sources: normalized.sources && typeof normalized.sources === "object" ? { ...normalized.sources } : {},
    overview: normalized.overview && typeof normalized.overview === "object" ? { ...normalized.overview } : {},
    crash_summary: {
      ...crashSummary,
      last_relevant_context: Array.isArray(crashSummary.last_relevant_context) ? crashSummary.last_relevant_context : [],
    },
    active_mods: Array.isArray(normalized.active_mods) ? normalized.active_mods : [],
    suspected_mods: normalizeDiagnosticsItemArray(normalized.suspected_mods, ["reasons", "matched_paths", "category_hints"]),
    missing_references: normalizeDiagnosticsItemArray(normalized.missing_references),
    errors: normalizeDiagnosticsItemArray(normalized.errors),
    logs: normalized.logs && typeof normalized.logs === "object" ? { ...normalized.logs } : {},
    removed_mod_suspected: Boolean(normalized.removed_mod_suspected),
    removed_mod_reason: safeValue(normalized.removed_mod_reason, ""),
    unreadable_mods: Array.isArray(normalized.unreadable_mods) ? normalized.unreadable_mods : [],
    limitations: Array.isArray(normalized.limitations) ? normalized.limitations : [],
    raw_relevant_log_lines: Array.isArray(normalized.raw_relevant_log_lines) ? normalized.raw_relevant_log_lines : [],
    raw_relevant_crash_lines: Array.isArray(normalized.raw_relevant_crash_lines) ? normalized.raw_relevant_crash_lines : [],
  };
}

function diagnosticsLabel(map, value) {
  return map?.[value] || safeValue(value, "-");
}

function formatDiagnosticsCategory(value) {
  return safeValue(String(value ?? "").replace(/([a-z])([A-Z])/g, "$1 $2"), "-");
}

function diagnosticsLine(message) {
  return `<p class="diagnostics-line">${escapeHtml(message)}</p>`;
}

function diagnosticsEmptyMessage(message) {
  return `<p class="diagnostics-empty-copy">${escapeHtml(message)}</p>`;
}

function diagnosticsBadge(label, tone = "neutral") {
  return `<span class="diagnostics-badge" data-tone="${escapeHtml(tone)}">${escapeHtml(safeValue(label, "-"))}</span>`;
}

function diagnosticsMetaLine(label, value, monospace = false) {
  if (!value) return "";
  return `
    <div class="diagnostics-meta-line">
      <span>${escapeHtml(label)}</span>
      <strong class="${monospace ? "detail-card-value--mono" : ""}">${escapeHtml(value)}</strong>
    </div>
  `;
}

function diagnosticsListItem(title, body, badges = [], extra = "") {
  const safeTitle = safeValue(title, "-");
  const safeBody = safeValue(body, "");
  return `
    <article class="diagnostics-item">
      <div class="diagnostics-item-head">
        <strong class="diagnostics-item-title">${escapeHtml(safeTitle)}</strong>
        ${badges.length ? `<div class="diagnostics-chip-row">${badges.join("")}</div>` : ""}
      </div>
      <p class="diagnostics-item-copy">${escapeHtml(safeBody)}</p>
      ${extra}
    </article>
  `;
}

async function copyTextToClipboard(text) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "absolute";
  textarea.style.left = "-9999px";
  document.body.appendChild(textarea);
  textarea.select();
  const succeeded = document.execCommand("copy");
  textarea.remove();
  if (!succeeded) {
    throw new Error("Clipboard write failed");
  }
}

function buildDiagnosticsSummaryText(report, copy) {
  const topSuspect = (report.suspected_mods || [])[0];
  const lines = [
    `Status: ${diagnosticsLabel(copy.status, report.overview.status_badge)}`,
    report.overview.summary,
    report.overview.confidence_note,
    report.overview.disclaimer,
    "",
    `Sources: game.log.txt ${report.sources.game_log_found ? copy.availabilityFound : copy.availabilityMissing}, game.crash.txt ${report.sources.game_crash_found ? copy.availabilityFound : copy.availabilityMissing}, mod folder ${report.sources.mod_folder_found ? copy.availabilityFound : copy.availabilityMissing}`,
    `Indexed mods: ${report.sources.indexed_mods_count} (unreadable: ${report.sources.unreadable_mods_count})`,
    `Extracted errors: ${report.sources.extracted_errors_count}`,
    "",
    `Crash Summary: ${safeValue(report.crash_summary.headline)}`,
    safeValue(report.crash_summary.summary),
    `Primary category: ${formatDiagnosticsCategory(report.crash_summary.primary_category || "UnknownReference")}`,
    `Errors: ${report.crash_summary.error_count} | Warnings: ${report.crash_summary.warning_count}`,
  ];

  if (topSuspect) {
    lines.push("", `Top suspect: ${topSuspect.name} (${diagnosticsLabel(copy.confidence, topSuspect.confidence)}, ${topSuspect.score}/100)`);
  } else if (report.removed_mod_suspected) {
    lines.push("", report.removed_mod_reason || copy.removedSuspected);
  }

  if ((report.missing_references || []).length) {
    lines.push("", "Missing / Removed References:");
    for (const item of report.missing_references.slice(0, 5)) {
      lines.push(`- [${formatDiagnosticsCategory(item.category)}] ${item.path}`);
    }
  }

  if ((report.limitations || []).length) {
    lines.push("", "Limitations:");
    for (const limitation of report.limitations) {
      lines.push(`- ${limitation}`);
    }
  }

  return lines.filter((line) => line !== undefined && line !== null).join("\n");
}

function buildDiagnosticsModsText(report, copy) {
  const suspects = report.suspected_mods || [];
  if (!suspects.length) {
    return report.removed_mod_reason || copy.noSuspectedMods;
  }

  return suspects
    .map((item, index) => {
      const lines = [
        `${index + 1}. ${item.name}`,
        `   ${diagnosticsLabel(copy.confidence, item.confidence)} | ${diagnosticsLabel(copy.activeState, item.active_state)} | ${item.score}/100`,
      ];

      if (item.package_name) lines.push(`   ${copy.fieldPackage}: ${item.package_name}`);
      if (item.file_path) lines.push(`   ${copy.fieldFile}: ${item.file_path}`);
      if (Array.isArray(item.category_hints) && item.category_hints.length) {
        lines.push(`   ${copy.fieldCategories}: ${item.category_hints.map(formatDiagnosticsCategory).join(", ")}`);
      }

      for (const reason of item.reasons || []) {
        lines.push(`   - ${reason}`);
      }
      for (const matchedPath of item.matched_paths || []) {
        lines.push(`   ${copy.fieldPath}: ${matchedPath}`);
      }

      return lines.join("\n");
    })
    .join("\n\n");
}

function diagnosticsErrorMatchesFilter(item, filterValue) {
  if (filterValue === "all") return true;
  return String(item?.severity || "").toLowerCase() === filterValue;
}

function renderDiagnosticsErrorList(report, copy) {
  if (!diagnosticsErrorsList) return;
  const errors = Array.isArray(report?.errors) ? report.errors : [];
  const filteredErrors = errors
    .filter((item) => diagnosticsErrorMatchesFilter(item, currentDiagnosticsSeverityValue))
    .slice(0, MAX_RENDERED_DIAGNOSTICS_ITEMS);

  if (!filteredErrors.length) {
    diagnosticsErrorsList.innerHTML = diagnosticsEmptyMessage(copy.noErrors);
    return;
  }

  const grouped = new Map();
  for (const item of filteredErrors) {
    const key = item.category || "UnknownReference";
    const list = grouped.get(key) || [];
    list.push(item);
    grouped.set(key, list);
  }

  diagnosticsErrorsList.innerHTML = Array.from(grouped.entries())
    .map(([category, items]) => {
      const renderedItems = items
        .map((item) => diagnosticsListItem(
          item.extracted_path || `${item.source}:${item.line_number || "-"}`,
          item.explanation || item.raw_line,
          [
            diagnosticsBadge(diagnosticsLabel(copy.severity, item.severity), diagnosticsBadgeTone(item.severity)),
            diagnosticsBadge(formatDiagnosticsCategory(item.category), "neutral"),
            diagnosticsBadge(item.source, "neutral"),
            item.in_last_context ? diagnosticsBadge(copy.lastContextLabel, "warning") : "",
          ].filter(Boolean),
          [
            item.extracted_path ? diagnosticsMetaLine(copy.fieldPath, item.extracted_path, true) : "",
            item.line_number ? diagnosticsMetaLine(copy.fieldSource, `${item.source}:${item.line_number}`, true) : diagnosticsMetaLine(copy.fieldSource, item.source),
            `<details class="diagnostics-raw-details"><summary>${escapeHtml(copy.rawLineLabel)}</summary><pre>${escapeHtml(item.raw_line || "-")}</pre></details>`,
          ].join("")
        ))
        .join("");

      return `
        <section class="diagnostics-group">
          <div class="diagnostics-group-head">
            <strong>${escapeHtml(formatDiagnosticsCategory(category))}</strong>
            ${diagnosticsBadge(String(items.length), "neutral")}
          </div>
          <div class="diagnostics-list diagnostics-list--compact">${renderedItems}</div>
        </section>
      `;
    })
    .join("");
}

function renderDiagnosticsSeverityFilter(copy) {
  if (!diagnosticsSeverityFilter) return;
  diagnosticsSeverityFilter.innerHTML = `
    <option value="all">${escapeHtml(copy.filterAll)}</option>
    <option value="info">${escapeHtml(copy.filterInfo)}</option>
    <option value="warning">${escapeHtml(copy.filterWarning)}</option>
    <option value="error">${escapeHtml(copy.filterError)}</option>
    <option value="critical">${escapeHtml(copy.filterCritical)}</option>
  `;
  diagnosticsSeverityFilter.value = currentDiagnosticsSeverityValue;
}

function renderDiagnosticsReport(report, copy) {
  const overview = report?.overview || {};
  const sources = report?.sources || {};
  const crashSummary = report?.crash_summary || {};
  const context = report?.context || {};
  const suspectedMods = Array.isArray(report?.suspected_mods)
    ? report.suspected_mods.slice(0, MAX_RENDERED_DIAGNOSTICS_ITEMS)
    : [];
  const missingReferences = Array.isArray(report?.missing_references)
    ? report.missing_references.slice(0, MAX_RENDERED_DIAGNOSTICS_ITEMS)
    : [];
  const unreadableMods = Array.isArray(report?.unreadable_mods)
    ? report.unreadable_mods.slice(0, MAX_RENDERED_DIAGNOSTICS_ITEMS)
    : [];
  const limitations = Array.isArray(report?.limitations)
    ? report.limitations.slice(0, MAX_RENDERED_DIAGNOSTICS_ITEMS)
    : [];
  const topSuspect = suspectedMods[0] || null;

  if (modalConflictDiagnosticsHeadline) modalConflictDiagnosticsHeadline.textContent = diagnosticsLabel(copy.status, overview.status_badge);
  if (modalConflictDiagnosticsSummary) modalConflictDiagnosticsSummary.textContent = safeValue(overview.summary);
  if (modalConflictDiagnosticsGuidance) {
    modalConflictDiagnosticsGuidance.textContent = [safeValue(overview.confidence_note), safeValue(overview.disclaimer)]
      .filter(Boolean)
      .join(" ");
  }

  if (diagnosticsSourcesGrid) {
    diagnosticsSourcesGrid.innerHTML = [
      diagnosticsMetaLine(copy.sourceGameLog, `${diagnosticsLabel(copy.availability, sources.game_log_found ? "found" : "missing")} | ${safeValue(sources.game_log_path, "-")}`, true),
      diagnosticsMetaLine(copy.sourceGameCrash, `${diagnosticsLabel(copy.availability, sources.game_crash_found ? "found" : "missing")} | ${safeValue(sources.game_crash_path, "-")}`, true),
      diagnosticsMetaLine(copy.sourceModFolder, `${diagnosticsLabel(copy.availability, sources.mod_folder_found ? "found" : "missing")} | ${safeValue(sources.mod_folder_path, "-")}`, true),
      diagnosticsMetaLine(copy.sourceIndexedMods, `${sources.indexed_mods_count ?? 0} | readable ${sources.readable_mods_count ?? 0} | unreadable ${sources.unreadable_mods_count ?? 0}`),
      diagnosticsMetaLine(copy.sourceActiveMods, `${sources.active_mods_count ?? 0} | ${sources.active_mods_reliably_known ? diagnosticsLabel(copy.availability, "found") : diagnosticsLabel(copy.availability, "missing")}`),
      diagnosticsMetaLine(copy.sourceExtractedErrors, String(sources.extracted_errors_count ?? 0)),
    ].join("");
  }

  if (diagnosticsContextGrid) {
    diagnosticsContextGrid.innerHTML = [
      diagnosticsMetaLine(copy.contextGame, context.selected_game?.toUpperCase()),
      diagnosticsMetaLine(copy.contextBasePath, context.base_path, true),
      diagnosticsMetaLine(copy.contextProfile, context.profile_path, true),
      diagnosticsMetaLine(copy.contextSave, context.save_path, true),
      diagnosticsMetaLine(copy.fieldProfileInferred, String(Boolean(context.profile_inferred))),
      diagnosticsMetaLine(copy.fieldSaveInferred, String(Boolean(context.save_inferred))),
    ].join("");
  }

  if (diagnosticsCrashPrimary) {
    diagnosticsCrashPrimary.textContent = safeValue(
      formatDiagnosticsCategory(crashSummary.primary_category || "UnknownReference"),
      "-"
    );
  }
  if (diagnosticsCrashSummary) diagnosticsCrashSummary.textContent = safeValue(crashSummary.summary);
  if (diagnosticsCrashCounts) {
    diagnosticsCrashCounts.innerHTML = [
      diagnosticsBadge(`${copy.severityError}: ${crashSummary.error_count ?? 0}`, diagnosticsBadgeTone("Error")),
      diagnosticsBadge(`${copy.severityWarning}: ${crashSummary.warning_count ?? 0}`, diagnosticsBadgeTone("Warning")),
      diagnosticsBadge(
        crashSummary.crash_detected ? diagnosticsLabel(copy.availability, "found") : diagnosticsLabel(copy.availability, "missing"),
        diagnosticsBadgeTone(crashSummary.crash_detected ? "found" : "missing")
      ),
    ].join("");
  }
  if (diagnosticsCrashContext) {
    diagnosticsCrashContext.innerHTML = (crashSummary.last_relevant_context || []).length
      ? crashSummary.last_relevant_context
        .slice(0, 8)
        .map((item) => diagnosticsLine(item))
        .join("")
      : diagnosticsEmptyMessage(copy.noCrashContext);
  }

  if (diagnosticsSuspectedState) {
    diagnosticsSuspectedState.textContent = topSuspect
      ? `${topSuspect.name} | ${diagnosticsLabel(copy.confidence, topSuspect.confidence)} | ${topSuspect.score}/100`
      : report.removed_mod_suspected
        ? safeValue(report.removed_mod_reason, copy.removedSuspected)
        : copy.noModAssigned;
  }
  if (diagnosticsSuspectedMods) {
    diagnosticsSuspectedMods.innerHTML = suspectedMods.length
      ? suspectedMods.map((item) => diagnosticsListItem(
        item.name,
        (item.reasons || []).slice(0, 3).join(" ") || copy.noSuspectedMods,
        [
          diagnosticsBadge(`${item.score}/100`, diagnosticsBadgeTone(item.confidence)),
          diagnosticsBadge(diagnosticsLabel(copy.confidence, item.confidence), diagnosticsBadgeTone(item.confidence)),
          diagnosticsBadge(diagnosticsLabel(copy.activeState, item.active_state), diagnosticsBadgeTone(item.active_state)),
        ],
        [
          diagnosticsMetaLine(copy.fieldPackage, item.package_name),
          diagnosticsMetaLine(copy.fieldFile, item.file_path, true),
          diagnosticsMetaLine(copy.fieldReadable, item.readable ? copy.availabilityFound : copy.availabilityMissing),
          diagnosticsMetaLine(copy.fieldManifest, String(Boolean(item.manifest_present))),
          Array.isArray(item.category_hints) && item.category_hints.length
            ? diagnosticsMetaLine(copy.fieldCategories, item.category_hints.map((value) => formatDiagnosticsCategory(value)).join(", "))
            : "",
          (item.matched_paths || []).length
            ? `<div class="diagnostics-chip-row">${item.matched_paths.slice(0, 5).map((asset) => diagnosticsBadge(asset, "neutral")).join("")}</div>`
            : "",
        ].join("")
      )).join("")
      : diagnosticsEmptyMessage(copy.noSuspectedMods);
  }

  if (diagnosticsMissingReferences) {
    diagnosticsMissingReferences.innerHTML = missingReferences.length
      ? missingReferences.map((item) => diagnosticsListItem(
        item.path,
        item.reason,
        [
          diagnosticsBadge(formatDiagnosticsCategory(item.category), "neutral"),
          diagnosticsBadge(item.source, "neutral"),
        ],
        [
          diagnosticsMetaLine(copy.fieldPath, item.path, true),
          diagnosticsMetaLine(copy.fieldSource, item.source),
        ].join("")
      )).join("")
      : diagnosticsEmptyMessage(copy.noMissingReferences);
  }

  renderDiagnosticsSeverityFilter(copy);
  renderDiagnosticsErrorList(report, copy);

  if (diagnosticsLogsInfo) {
    const logItems = [
      diagnosticsMetaLine(copy.logsTechnical, report.logs?.technical_log_path, true),
      diagnosticsMetaLine(copy.logsUser, report.logs?.user_log_path, true),
      diagnosticsMetaLine(copy.logsFolder, report.logs?.log_directory_path, true),
    ].filter(Boolean);
    diagnosticsLogsInfo.innerHTML = logItems.length ? logItems.join("") : diagnosticsEmptyMessage(copy.noLogs);
  }

  if (diagnosticsLimitations) {
    const renderedLimitations = [];
    if (unreadableMods.length) {
      renderedLimitations.push(diagnosticsListItem(
        copy.protectedModsTitle,
        copy.protectedModsHint,
        [diagnosticsBadge(String(unreadableMods.length), "warning")],
        `<div class="diagnostics-chip-row">${unreadableMods.slice(0, 12).map((item) => diagnosticsBadge(item, "warning")).join("")}</div>`
      ));
    }
    if (sources.analysis_timed_out) {
      renderedLimitations.push(diagnosticsListItem(copy.limitLabel, copy.analysisTimedOut));
    }
    diagnosticsLimitations.innerHTML = renderedLimitations.length || limitations.length
      ? renderedLimitations.concat(limitations.map((item) => diagnosticsListItem(copy.limitLabel, item))).join("")
      : diagnosticsEmptyMessage(copy.noLimitations);
  }

  setModalPillState(
    modalConflictDiagnosticsConfidence,
    diagnosticsStatusState(overview.status_badge),
    diagnosticsLabel(copy.status, overview.status_badge)
  );
  setModalPillState(
    modalConflictDiagnosticsHealth,
    diagnosticsConfidenceState(topSuspect?.confidence || "Low"),
    topSuspect
      ? diagnosticsLabel(copy.confidence, topSuspect.confidence)
      : report.removed_mod_suspected
        ? copy.removedSuspected
        : diagnosticsLabel(copy.confidence, "Low")
  );
}

function setProfileShareStatus(state, title, message, path = "") {
  setModalPillState(profileShareStatusPill, state, title);
  profileShareStatusTitle.textContent = title;
  profileShareStatusMessage.textContent = message;
  profileShareLastPath.hidden = !path;
  profileShareLastPath.textContent = path || "";
}

function resolveStoredProfileSharePath() {
  const storedPath = localStorage.getItem("ets2_profile_share_profile_path");
  if (!storedPath) return null;
  const normalized = String(storedPath).trim();
  return normalized ? normalized : null;
}



/* --------------------------------------------------------------
   TEXT MODAL
-------------------------------------------------------------- */
export async function openModalText(titleKey, placeholderKey, initialValue = "") {
  modalTextTitle.textContent = await window.t(titleKey);
  modalTextInput.placeholder = await window.t(placeholderKey);
  modalTextInput.value = initialValue;
  modalText.style.display = "flex";

  console.log(`[app.js] Öffne Text-Modal: "${titleKey}"`);
  return new Promise((resolve) => {
    function apply() {
      const val = modalTextInput.value;
      cleanup();
      resolve(val);
    }
    function cancel() {
      cleanup();
      resolve(null);
    }
    function cleanup() {
      modalTextApply.removeEventListener("click", apply);
      modalTextCancel.removeEventListener("click", cancel);
      modalText.style.display = "none";
    }

    modalTextApply.addEventListener("click", apply);
    modalTextCancel.addEventListener("click", cancel);
  });
};

/* --------------------------------------------------------------
   NUMBER MODAL
-------------------------------------------------------------- */
export async function openModalNumber(titleKey, value = 0) {
  modalNumberTitle.textContent = await window.t(titleKey);
  modalNumberInput.value = value;
  modalNumber.style.display = "flex";

  console.log(`[app.js] Öffne Number-Modal: "${titleKey}" mit Wert ${value}`);
  return new Promise((resolve) => {
    function apply() {
      const val = Number(modalNumberInput.value);
      console.log("[app.js] Number-Modal 'Apply' geklickt, Wert:", val);
      cleanup();
      resolve(val);
    }
    function cancel() {
      cleanup();
      resolve(null);
    }
    function cleanup() {
      modalNumberApply.removeEventListener("click", apply);
      modalNumberCancel.removeEventListener("click", cancel);
      modalNumber.style.display = "none";
    }

    modalNumberApply.addEventListener("click", apply);
    modalNumberCancel.addEventListener("click", cancel);
  });
};

/* --------------------------------------------------------------
   SLIDER MODAL (Single 0/1)
-------------------------------------------------------------- */
export async function openModalSlider(titleKey, isChecked = 0) {
  modalSliderTitle.textContent = await window.t(titleKey);
  modalSliderInput.checked = Boolean(isChecked);
  const modalSliderToggle = modalSlider.querySelector(".toggle-switch");
  const modalSliderState = modalSlider.querySelector(".toggle-switch__state");
  const sliderOnLabel = await window.t("label.toggle_on");
  const sliderOffLabel = await window.t("label.toggle_off");

  const syncSliderState = () => {
    const isActive = Boolean(modalSliderInput.checked);
    modalSliderToggle?.classList.toggle("toggle-switch--active", isActive);
    if (modalSliderState) {
      modalSliderState.textContent = isActive ? sliderOnLabel : sliderOffLabel;
    }
  };

  modalSliderInput.addEventListener("change", syncSliderState);
  syncSliderState();
  modalSlider.style.display = "flex";

  console.log(`[app.js] Öffne Slider-Modal: "${titleKey}" mit Wert ${isChecked}`);
  return new Promise((resolve) => {
    function apply() {
      const val = modalSliderInput.checked ? 1 : 0;
      console.log("[app.js] Slider-Modal 'Apply' geklickt, Wert:", val);
      cleanup();
      resolve(val);
    }
    function cancel() {
      cleanup();
      resolve(null);
    }
    function cleanup() {
      modalSliderApply.removeEventListener("click", apply);
      modalSliderCancel.removeEventListener("click", cancel);
      modalSliderInput.removeEventListener("change", syncSliderState);
      modalSlider.style.display = "none";
    }

    modalSliderApply.addEventListener("click", apply);
    modalSliderCancel.addEventListener("click", cancel);
  });
};

/* --------------------------------------------------------------
   MULTI-MODAL (NUMBER, SLIDER, DROPDOWN, ADR, CHECKBOX)
-------------------------------------------------------------- */
export async function openModalMulti(titleKey, config = []) {
  modalMultiTitle.textContent = await window.t(titleKey);
  modalMultiContent.innerHTML = "";

  console.log(`[app.js] Öffne Multi-Modal: "${titleKey}"`);
  const adrLevels = [1, 3, 7, 15, 31, 63];
  const toggleOnLabel = await window.t("label.toggle_on");
  const toggleOffLabel = await window.t("label.toggle_off");

  const inputs = [];

  for (const item of config) {
    const row = document.createElement("div");
    row.className = "modal-row";

    const label = document.createElement("div");
    label.className = "modal-label";
    label.textContent = await window.t(item.label);

    const control = document.createElement("div");
    control.className = "modal-control";

    /* NUMBER */
    if (item.type === "number") {
      const input = document.createElement("input");
      input.type = "number";
      input.id = item.id;
      input.value = item.value ?? 0;
      input.className = "modal-number";
      control.appendChild(input);
      inputs.push(input);
    }

    /* DROPDOWN */
    if (item.type === "dropdown") {
      const select = document.createElement("select");
      select.id = item.id;
      select.className = "modal-dropdown";

      for (const o of item.options) {
        const opt = document.createElement("option");
        opt.value = o;
        opt.textContent = item.optionLabels?.[String(o)] ?? await window.t(o);
        if (String(o) === String(item.value)) opt.selected = true;
        select.appendChild(opt);
      }

      control.appendChild(select);
      inputs.push(select);
    }

    /* SLIDER / ADR */
    if (item.type === "slider" || item.type === "adr") {
      const val = document.createElement("span");
      val.id = `${item.id}_val`;
      val.className = "slider-value";

      const slider = document.createElement("input");
      slider.type = "range";

      if (item.type === "adr") {
        slider.min = 0;
        slider.max = adrLevels.length - 1;
        slider.value = adrLevels.indexOf(item.value) ?? 0;
        val.textContent = adrLevels[slider.value];

        slider.addEventListener("input", () => {
          val.textContent = adrLevels[slider.value];
        });
      } else {
        slider.min = item.min ?? 0;
        slider.max = item.max ?? 6;
        slider.step = item.step ?? 1;
        slider.value = item.value ?? 0;

        val.textContent = slider.value;


        slider.addEventListener("input", () => {
          val.textContent = slider.value;
        });
      }

      slider.id = item.id;
      slider.className = "skill-slider";
      control.appendChild(val);
      control.appendChild(slider);
      inputs.push(slider);
    }

    /* CHECKBOX */
    if (item.type === "checkbox") {
      const switchRow = document.createElement("div");
      switchRow.className = "setting-toggle-row";

      const copy = document.createElement("div");
      copy.className = "setting-toggle-copy";

      const toggleLabel = document.createElement("label");
      toggleLabel.className = "setting-toggle-label";
      toggleLabel.htmlFor = item.id;
      toggleLabel.textContent = await window.t(item.label);

      copy.appendChild(toggleLabel);

      if (item.description) {
        const description = document.createElement("p");
        description.className = "setting-toggle-description";
        description.textContent = await window.t(item.description);
        copy.appendChild(description);
      }

      const toggleSwitch = document.createElement("label");
      toggleSwitch.className = "toggle-switch";

      const input = document.createElement("input");
      input.type = "checkbox";
      input.id = item.id;
      input.checked = Boolean(item.value ?? 0);
      input.className = "toggle-switch__input";
      input.setAttribute("aria-label", await window.t(item.label));

      const slider = document.createElement("span");
      slider.className = "toggle-switch__slider";
      slider.setAttribute("aria-hidden", "true");

      const state = document.createElement("span");
      state.className = "toggle-switch__state";

      const syncToggleState = () => {
        const isActive = Boolean(input.checked);
        toggleSwitch.classList.toggle("toggle-switch--active", isActive);
        state.textContent = isActive ? toggleOnLabel : toggleOffLabel;
      };

      input.addEventListener("change", syncToggleState);
      syncToggleState();

      toggleSwitch.appendChild(input);
      toggleSwitch.appendChild(slider);
      toggleSwitch.appendChild(state);

      switchRow.appendChild(copy);
      switchRow.appendChild(toggleSwitch);
      row.appendChild(switchRow);
      inputs.push(input);
      modalMultiContent.appendChild(row);
      continue;
    }

    row.appendChild(label);
    row.appendChild(control);
    modalMultiContent.appendChild(row);
  }

  modalMulti.style.display = "flex";

  return new Promise((resolve) => {
    function apply() {
      const result = {};
      inputs.forEach((i) => {
        if (i.type === "range" && config.find(c => c.id === i.id)?.type === "adr") {
          const val = adrLevels[i.value];
          result[i.id] = val;
        } else if (i.type === "range" || i.type === "number") {
          result[i.id] = Number(i.value);
        } else if (i.type === "checkbox") {
          result[i.id] = i.checked ? 1 : 0;
        } else {
          result[i.id] = i.value;
        }
      });

      // console.log("[app.js] Multi-Modal 'Apply' geklickt, Werte:", result);
      cleanup();
      resolve(result);
    }

    function cancel() {
      cleanup();
      resolve(null);
    }

    function cleanup() {
      modalMultiApplyBtn.removeEventListener("click", apply);
      modalMultiCancelBtn.removeEventListener("click", cancel);
      modalMulti.style.display = "none";
    }

    modalMultiApplyBtn.addEventListener("click", apply);
    modalMultiCancelBtn.addEventListener("click", cancel);
  });
};

/* --------------------------------------------------------------
   CLONE PROFILE MODAL
-------------------------------------------------------------- */
export async function openCloneProfileModal() {
  if (!window.selectedProfilePath) {
    window.showToast("toasts.profile_not_selected", "warning");
    return;
  }

  // Reset UI
  cloneNameInput.value = "";
  cloneNameInput.placeholder = await window.t("modals.clone_profile.new_name_placeholder");
  cloneValidationMsg.textContent = "";
  cloneBackup.checked = true;
  modalCloneApply.disabled = true;
  modalCloneApply.textContent = await window.t("modals.clone_profile.clone_button");
  
  const profileName = document.querySelector("#profileNameDisplay")?.textContent || "Unknown";
  cloneSourceDisplay.textContent = (await window.t("modals.clone_profile.source")).replace('{profileName}', profileName);

  modalClone.style.display = "flex";
  cloneNameInput.focus();

  let debounceTimer;

  async function validate() {
    const newName = cloneNameInput.value.trim();
    modalCloneApply.disabled = true;
    cloneValidationMsg.textContent = await window.t("modals.clone_profile.validation_checking");
    cloneValidationMsg.style.color = "#aaa";

    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(async () => {
      if (!newName) {
        cloneValidationMsg.textContent = "";
        return;
      }

      try {
        const status = await window.invoke("validate_clone_target", {
          sourceProfile: window.selectedProfilePath,
          newName: newName,
        });

        if (status.valid) {
          cloneValidationMsg.textContent = (await window.t("modals.clone_profile.validation_valid")).replace('{message}', status.message);
          cloneValidationMsg.style.color = "#4caf50";
          modalCloneApply.disabled = false;
        } else {
          cloneValidationMsg.textContent = (await window.t("modals.clone_profile.validation_invalid")).replace('{message}', status.message);
          cloneValidationMsg.style.color = "#f44336";
        }
      } catch (e) {
        cloneValidationMsg.textContent = `Error: ${e}`;
        cloneValidationMsg.style.color = "#f44336";
      }
    }, 300);
  }

  async function apply() {
    if (window.logUserAction) window.logUserAction("clone_profile", "start");
    const newName = cloneNameInput.value.trim();
    if (!newName) return;

    modalCloneApply.disabled = true;
    modalCloneApply.textContent = await window.t("modals.clone_profile.cloning_button");

    try {
      const msg = await window.invoke("clone_profile_command", {
        sourceProfile: window.selectedProfilePath,
        newName,
        backup: true,
        replaceHex: true,
        replaceText: true,
      });

      window.showToast(msg, "success");
      if (window.logUserAction) window.logUserAction("clone_profile", "success");
      
      // Refresh list
      const refreshBtn = document.querySelector("#refreshBtn");
      if (refreshBtn) refreshBtn.click();

      cleanup();
    } catch (e) {
      if (window.logUserAction) window.logUserAction("clone_profile", "error");
      window.showToast((await window.t("toasts.clone_failed")).replace('{error}', e), "error");
      console.error(e);
      modalCloneApply.disabled = false;
      modalCloneApply.textContent = await window.t("modals.clone_profile.clone_button");
    }
  }

  function cancel() {
    cleanup();
  }

  function cleanup() {
    cloneNameInput.removeEventListener("input", validate);
    modalCloneApply.removeEventListener("click", apply);
    modalCloneCancel.removeEventListener("click", cancel);
    modalClone.style.display = "none";
  }

  cloneNameInput.addEventListener("input", validate);
  modalCloneApply.addEventListener("click", apply);
  modalCloneCancel.addEventListener("click", cancel);
}

/* --------------------------------------------------------------
   LEVEL / XP MODAL
-------------------------------------------------------------- */
export async function openLevelSystemModal() {
  if (!modalLevelSystem) return;

  if (!window.selectedProfilePath) {
    window.showToast("toasts.profile_not_selected", "warning");
    return;
  }

  if (!window.selectedSavePath) {
    window.showToast("toasts.level_system_missing_data", "warning");
    return;
  }

  if ((!window.currentProfileData || window.currentProfileData.xp === undefined) && typeof window.loadProfileData === "function") {
    try {
      await window.loadProfileData();
    } catch (error) {
      console.error("Level system data load failed:", error);
      window.showToast("toasts.level_system_missing_data", "error");
      return;
    }
  }

  const table = await loadLevelTable();
  if (!table.length) {
    window.showToast("toasts.level_system_load_error", "error");
    return;
  }

  const copy = {
    modeLevel: await window.t("modals.level_system.mode_level"),
    modeXp: await window.t("modals.level_system.mode_xp"),
    statusCurrent: await window.t("modals.level_system.status.current"),
    statusTarget: await window.t("modals.level_system.status.target"),
    statusApplying: await window.t("modals.level_system.status.applying"),
    previewEntered: await window.t("modals.level_system.preview.entered"),
    previewCalculated: await window.t("modals.level_system.preview.calculated"),
    feedbackLevelClamped: await window.t("modals.level_system.feedback.level_clamped", {
      min: 0,
      max: getMaxLevel(table),
    }),
    feedbackXpClamped: await window.t("modals.level_system.feedback.xp_clamped", {
      min: 0,
      maxXp: formatMetric(getXpForLevel(getMaxLevel(table), table), 0),
    }),
    feedbackRequired: await window.t("modals.level_system.feedback.required"),
    feedbackInvalid: await window.t("modals.level_system.feedback.invalid"),
    feedbackUnchanged: await window.t("modals.level_system.feedback.unchanged"),
    feedbackApplyError: await window.t("modals.level_system.feedback.apply_error"),
  };

  const maxLevel = getMaxLevel(table);
  const maxXp = getXpForLevel(maxLevel, table);
  const currentRawXp = Math.max(0, Math.floor(Number(window.currentProfileData?.xp ?? 0)));
  const currentXp = Number.isFinite(currentRawXp) ? currentRawXp : 0;
  const currentLevel = getLevelForXp(currentXp, table);
  const currentLevelXp = getXpForLevel(currentLevel, table);
  const initialMode = currentXp === currentLevelXp ? "level" : "xp";
  const state = {
    mode: initialMode,
    currentXp,
    currentLevel,
    targetLevel: currentLevel,
    targetXp: currentXp,
    levelInput: String(currentLevel),
    xpInput: String(currentXp),
    valid: true,
    applying: false,
    hint: "",
  };

  const parseIntegerInput = (value) => {
    const raw = String(value ?? "").trim();
    if (!raw) {
      return { valid: false, reason: "required", value: 0 };
    }

    const numeric = Number(raw);
    if (!Number.isFinite(numeric)) {
      return { valid: false, reason: "invalid", value: 0 };
    }

    return { valid: true, value: Math.floor(numeric) };
  };

  const hasTargetChange = () => state.targetLevel !== state.currentLevel || state.targetXp !== state.currentXp;

  const setMode = (mode) => {
    state.mode = mode === "xp" ? "xp" : "level";
    if (state.mode === "level") {
      state.targetLevel = clampLevel(state.targetLevel, table);
      state.targetXp = getXpForLevel(state.targetLevel, table);
      state.levelInput = String(state.targetLevel);
    } else {
      state.targetXp = clampXp(state.targetXp, table);
      state.targetLevel = getLevelForXp(state.targetXp, table);
      state.xpInput = String(state.targetXp);
    }
    state.valid = true;
    state.hint = "";
  };

  const updateFromLevel = (value, { inputOrigin = false } = {}) => {
    state.mode = "level";
    state.levelInput = String(value ?? "");
    const parsed = parseIntegerInput(value);
    if (!parsed.valid) {
      state.valid = false;
      state.hint = parsed.reason === "required" ? copy.feedbackRequired : copy.feedbackInvalid;
      render();
      return;
    }

    const clampedLevel = clampLevel(parsed.value, table);
    state.valid = true;
    state.targetLevel = clampedLevel;
    state.targetXp = getXpForLevel(clampedLevel, table);
    state.xpInput = String(state.targetXp);
    state.hint = inputOrigin && clampedLevel !== parsed.value
      ? copy.feedbackLevelClamped
      : "";
    render();
  };

  const updateFromXp = (value, { inputOrigin = false } = {}) => {
    state.mode = "xp";
    state.xpInput = String(value ?? "");
    const parsed = parseIntegerInput(value);
    if (!parsed.valid) {
      state.valid = false;
      state.hint = parsed.reason === "required" ? copy.feedbackRequired : copy.feedbackInvalid;
      render();
      return;
    }

    const clampedTargetXp = clampXp(parsed.value, table);
    state.valid = true;
    state.targetXp = clampedTargetXp;
    state.targetLevel = getLevelForXp(clampedTargetXp, table);
    state.levelInput = String(state.targetLevel);
    state.hint = inputOrigin && clampedTargetXp !== parsed.value
      ? copy.feedbackXpClamped
      : "";
    render();
  };

  const render = () => {
    const changed = hasTargetChange();
    const statusState = state.applying ? "loading" : state.valid ? "success" : "error";
    const statusText = state.applying
      ? copy.statusApplying
      : state.mode === "level"
        ? copy.modeLevel
        : copy.modeXp;

    setModalPillState(
      modalLevelSystemModePill,
      statusState,
      statusText
    );

    if (levelSystemCurrentStatus) levelSystemCurrentStatus.textContent = copy.statusCurrent;
    if (levelSystemTargetStatus) levelSystemTargetStatus.textContent = copy.statusTarget;

    if (levelSystemCurrentBadge) levelSystemCurrentBadge.textContent = formatLevelBadge(state.currentLevel);
    if (levelSystemCurrentXp) levelSystemCurrentXp.textContent = formatMetric(state.currentXp, 0);

    if (levelSystemTargetBadge) levelSystemTargetBadge.textContent = state.valid ? formatLevelBadge(state.targetLevel) : "-";
    if (levelSystemTargetXp) levelSystemTargetXp.textContent = state.valid ? formatMetric(state.targetXp, 0) : "-";
    if (levelSystemTargetLevelMeta) {
      levelSystemTargetLevelMeta.textContent = state.mode === "xp" ? copy.previewCalculated : copy.previewEntered;
      levelSystemTargetLevelMeta.classList.toggle("is-calculated", state.mode === "xp");
    }
    if (levelSystemTargetXpMeta) {
      levelSystemTargetXpMeta.textContent = state.mode === "level" ? copy.previewCalculated : copy.previewEntered;
      levelSystemTargetXpMeta.classList.toggle("is-calculated", state.mode === "level");
    }

    if (levelSystemModeLevel) {
      levelSystemModeLevel.classList.toggle("is-active", state.mode === "level");
      levelSystemModeLevel.setAttribute("aria-pressed", state.mode === "level" ? "true" : "false");
    }
    if (levelSystemModeXp) {
      levelSystemModeXp.classList.toggle("is-active", state.mode === "xp");
      levelSystemModeXp.setAttribute("aria-pressed", state.mode === "xp" ? "true" : "false");
    }

    if (levelSystemLevelField) {
      levelSystemLevelField.hidden = false;
      levelSystemLevelField.classList.toggle("is-active", state.mode === "level");
    }
    if (levelSystemXpField) {
      levelSystemXpField.hidden = false;
      levelSystemXpField.classList.toggle("is-active", state.mode === "xp");
    }

    if (levelSystemLevelInput) {
      levelSystemLevelInput.value = state.levelInput;
      levelSystemLevelInput.min = "0";
      levelSystemLevelInput.max = String(maxLevel);
      levelSystemLevelInput.disabled = state.applying;
    }
    if (levelSystemXpInput) {
      levelSystemXpInput.value = state.xpInput;
      levelSystemXpInput.min = "0";
      levelSystemXpInput.max = String(maxXp);
      levelSystemXpInput.disabled = state.applying;
    }

    if (modalLevelSystemApply) {
      modalLevelSystemApply.disabled = state.applying || !state.valid || !changed;
    }

    if (!state.hint && !changed) {
      setLevelSystemHint(copy.feedbackUnchanged);
      return;
    }

    setLevelSystemHint(state.hint);
  };

  modalLevelSystem.style.display = "flex";

  function cleanup() {
    modalLevelSystemClose?.removeEventListener("click", cleanup);
    modalLevelSystem?.removeEventListener("click", handleBackdropClick);
    levelSystemModeLevel?.removeEventListener("click", handleModeLevel);
    levelSystemModeXp?.removeEventListener("click", handleModeXp);
    levelSystemLevelInput?.removeEventListener("focus", handleLevelInputFocus);
    levelSystemXpInput?.removeEventListener("focus", handleXpInputFocus);
    levelSystemLevelInput?.removeEventListener("input", handleLevelInputChange);
    levelSystemXpInput?.removeEventListener("input", handleXpInputChange);
    modalLevelSystemApply?.removeEventListener("click", handleApply);
    setLevelSystemHint("");
    modalLevelSystem.style.display = "none";
  }

  function handleBackdropClick(event) {
    if (event.target === modalLevelSystem) {
      cleanup();
    }
  }

  function handleModeLevel() {
    setMode("level");
    state.hint = "";
    render();
  }

  function handleModeXp() {
    setMode("xp");
    state.hint = "";
    render();
  }

  function handleLevelInputFocus() {
    if (state.mode !== "level") {
      setMode("level");
      render();
    }
  }

  function handleXpInputFocus() {
    if (state.mode !== "xp") {
      setMode("xp");
      render();
    }
  }

  function handleLevelInputChange(event) {
    updateFromLevel(event.target.value, { inputOrigin: true });
  }

  function handleXpInputChange(event) {
    updateFromXp(event.target.value, { inputOrigin: true });
  }

  async function handleApply() {
    if (!state.valid || !hasTargetChange()) {
      state.hint = state.valid ? copy.feedbackUnchanged : copy.feedbackInvalid;
      render();
      return;
    }

    state.applying = true;
    state.hint = "";
    render();

    try {
      await window.invoke("edit_level", { xp: state.targetXp });
      if (typeof window.loadProfileData === "function") {
        await window.loadProfileData();
      }
      if (window.currentProfileData) {
        window.currentProfileData.xp = state.targetXp;
      }
      if (typeof window.refreshOperationalOverview === "function") {
        await window.refreshOperationalOverview();
      }
      window.showToast("toasts.level_system_apply_success", {
        level: state.targetLevel,
        xp: formatMetric(state.targetXp, 0),
      }, "success");
      cleanup();
    } catch (error) {
      console.error("Level system apply failed:", error);
      window.showToast("toasts.level_system_apply_error", "error");
      state.applying = false;
      state.hint = String(error?.message || error || copy.feedbackApplyError);
      render();
    }
  }

  modalLevelSystemClose?.addEventListener("click", cleanup);
  modalLevelSystem?.addEventListener("click", handleBackdropClick);
  levelSystemModeLevel?.addEventListener("click", handleModeLevel);
  levelSystemModeXp?.addEventListener("click", handleModeXp);
  levelSystemLevelInput?.addEventListener("focus", handleLevelInputFocus);
  levelSystemXpInput?.addEventListener("focus", handleXpInputFocus);
  levelSystemLevelInput?.addEventListener("input", handleLevelInputChange);
  levelSystemXpInput?.addEventListener("input", handleXpInputChange);
  modalLevelSystemApply?.addEventListener("click", handleApply);

  render();
  requestAnimationFrame(() => {
    modalLevelSystem.querySelector(".level-system-scroll")?.scrollTo({ top: 0, left: 0 });
  });
}

/* --------------------------------------------------------------
   CURRENT TRUCK MODAL
-------------------------------------------------------------- */
export async function openCurrentTruckModal() {
  if (!window.selectedProfilePath) {
    window.showToast("toasts.profile_not_selected", "warning");
    return;
  }

  if (window.logUserAction) window.logUserAction("view_current_truck", "start");

  const loadingLabel = await window.t("modals.truck_info.loading");
  const readyLabel = await window.t("modals.truck_info.ready");
  const emptyLabel = await window.t("modals.truck_info.empty_state");
  const errorLabel = await window.t("modals.truck_info.error_state");
  const loadingErrorText = await window.t("modals.truck_info.error_body");

  resetTruckModalPanels();
  setModalPillState(modalTruckInfoState, "loading", loadingLabel);
  modalTruckInfoLoading.hidden = false;
  modalTruckInfoErrorText.textContent = "";
  modalTruckInfo.style.display = "flex";

  function cleanup() {
    modalTruckInfoClose.removeEventListener("click", cleanup);
    modalTruckInfo.removeEventListener("click", handleBackdropClick);
    document.removeEventListener("keydown", handleEscape);
    modalTruckInfo.style.display = "none";
  }

  function handleBackdropClick(event) {
    if (event.target === modalTruckInfo) {
      cleanup();
    }
  }

  function handleEscape(event) {
    if (event.key === "Escape") {
      cleanup();
    }
  }

  modalTruckInfoClose.addEventListener("click", cleanup);
  modalTruckInfo.addEventListener("click", handleBackdropClick);
  document.addEventListener("keydown", handleEscape);

  try {
    const truckSummary = await window.invoke("get_current_truck_summary");

    if (!truckSummary?.display_name && !truckSummary?.brand_label && !truckSummary?.model_label) {
      resetTruckModalPanels();
      modalTruckInfoEmpty.hidden = false;
      setModalPillState(modalTruckInfoState, "warning", emptyLabel);
      if (window.logUserAction) window.logUserAction("view_current_truck", "empty");
      return;
    }

    modalTruckInfoName.textContent = safeValue(
      truckSummary?.display_name || [truckSummary?.brand_label, truckSummary?.model_label].filter(Boolean).join(" "),
      "-"
    );
    modalTruckInfoBrand.textContent = safeValue(truckSummary?.brand_label);
    modalTruckInfoModel.textContent = safeValue(truckSummary?.model_label);
    modalTruckInfoOdometer.textContent = formatDistance(truckSummary?.odometer_km);

    resetTruckModalPanels();
    modalTruckInfoContent.hidden = false;
    setModalPillState(modalTruckInfoState, "success", readyLabel);
    if (window.logUserAction) window.logUserAction("view_current_truck", "success");
  } catch (error) {
    console.error("Current truck modal failed:", error);
    resetTruckModalPanels();
    modalTruckInfoError.hidden = false;
    modalTruckInfoErrorText.textContent = loadingErrorText;
    setModalPillState(modalTruckInfoState, "error", errorLabel);
    if (window.logUserAction) window.logUserAction("view_current_truck", "error");
  }
}

/* --------------------------------------------------------------
   MOD CONFLICT DIAGNOSTICS MODAL
-------------------------------------------------------------- */
export async function openModConflictDiagnosticsModal(options = {}) {
  if (!modalConflictDiagnostics) return;
  if (isModAnalysisRunning) {
    window.showToast("Mod analysis already running", "warning");
    return;
  }

  const openSurface = typeof options.openSurface === "function"
    ? options.openSurface
    : () => {
        modalConflictDiagnostics.style.display = "flex";
      };
  const closeSurface = typeof options.closeSurface === "function"
    ? options.closeSurface
    : () => {
        modalConflictDiagnostics.style.display = "none";
      };
  const allowBackdropClose = options.allowBackdropClose ?? true;
  const onClose = typeof options.onClose === "function" ? options.onClose : null;

  const sessionId = ++currentDiagnosticsSessionId;
  let copy = createDefaultDiagnosticsCopy();
  let activeRun = 0;

  function isStaleRun(runId) {
    return sessionId !== currentDiagnosticsSessionId || runId !== activeRun;
  }

  currentDiagnosticsReport = null;
  openSurface();
  await logDiagnosticsFrontendEvent("modal_opened", "Mod Conflict Analyzer opened", false);
  if (window.logUserAction) window.logUserAction("mod_conflict_analyzer", "start");

  async function exportDiagnosticsReport(formatted) {
    if (!currentDiagnosticsReport) return;
    try {
      const exportPath = await window.invoke("export_mod_conflict_diagnostics_report", {
        report: currentDiagnosticsReport,
        formatted,
      });
      if (!exportPath) return;
      window.showToast("toasts.diagnostics_export_success", { path: exportPath }, "success");
    } catch (error) {
      console.error("Diagnostics export failed:", error);
      await logDiagnosticsFrontendEvent(
        "export_failed",
        safeValue(error?.message || error, "Unknown export failure"),
        true
      );
      window.showToast("toasts.diagnostics_export_failed", "error");
    }
  }

  async function handleExportReport() {
    await exportDiagnosticsReport(true);
  }

  async function handleExportErrors() {
    await exportDiagnosticsReport(false);
  }

  async function handleExportCrash() {
    await exportDiagnosticsReport(true);
  }

  async function handleCopySummary() {
    if (!currentDiagnosticsReport) return;
    try {
      await copyTextToClipboard(buildDiagnosticsSummaryText(currentDiagnosticsReport, copy));
      window.showToast("toasts.diagnostics_copy_summary_success", "success");
    } catch (error) {
      console.error("Summary copy failed:", error);
      window.showToast("toasts.diagnostics_copy_failed", "error");
    }
  }

  async function handleOpenLogs() {
    const logDirectory = safeValue(currentDiagnosticsReport?.logs?.log_directory_path, "");
    if (!logDirectory) return;

    try {
      const tauriOpener = window.__TAURI__?.opener;
      if (typeof tauriOpener?.openPath === "function") {
        await tauriOpener.openPath(logDirectory);
        return;
      }

      await copyTextToClipboard(logDirectory);
      window.showToast("toasts.diagnostics_log_path_copied", "success");
    } catch (error) {
      console.error("Log folder open failed:", error);
      try {
        await copyTextToClipboard(logDirectory);
        window.showToast("toasts.diagnostics_log_path_copied", "success");
      } catch (copyError) {
        console.error("Log folder copy failed:", copyError);
        window.showToast("toasts.diagnostics_log_open_failed", "error");
      }
    }
  }

  async function handleRefresh() {
    if (isModAnalysisRunning) return;
    await runAnalysis();
  }

  function cleanup() {
    if (sessionId === currentDiagnosticsSessionId) {
      currentDiagnosticsSessionId += 1;
    }
    modalConflictDiagnosticsClose?.removeEventListener("click", cleanup);
    if (allowBackdropClose) {
      modalConflictDiagnostics.removeEventListener("click", handleBackdropClick);
    }
    diagnosticsRefreshBtn?.removeEventListener("click", handleRefresh);
    diagnosticsDeepScanBtn?.removeEventListener("click", handleDeepScan);
    diagnosticsRefreshFooterBtn?.removeEventListener("click", handleRefresh);
    diagnosticsDeepScanFooterBtn?.removeEventListener("click", handleDeepScan);
    diagnosticsExportReportBtn?.removeEventListener("click", handleExportReport);
    diagnosticsExportErrorsBtn?.removeEventListener("click", handleExportErrors);
    diagnosticsExportCrashBtn?.removeEventListener("click", handleExportCrash);
    diagnosticsCopySummaryBtn?.removeEventListener("click", handleCopySummary);
    diagnosticsOpenLogFolderBtn?.removeEventListener("click", handleOpenLogs);
    modalConflictDiagnosticsRetryBtn?.removeEventListener("click", handleRefresh);
    modalConflictDiagnosticsDeepBtn?.removeEventListener("click", handleDeepScan);
    diagnosticsSeverityFilter?.removeEventListener("change", handleSeverityFilterChange);
    currentDiagnosticsReport = null;
    closeSurface();
    onClose?.();
  }

  function handleBackdropClick(event) {
    if (event.target === modalConflictDiagnostics) {
      cleanup();
    }
  }

  modalConflictDiagnosticsClose?.addEventListener("click", cleanup);
  if (allowBackdropClose) {
    modalConflictDiagnostics.addEventListener("click", handleBackdropClick);
  }
  diagnosticsRefreshBtn?.addEventListener("click", handleRefresh);
  diagnosticsDeepScanBtn?.addEventListener("click", handleDeepScan);
  diagnosticsRefreshFooterBtn?.addEventListener("click", handleRefresh);
  diagnosticsDeepScanFooterBtn?.addEventListener("click", handleDeepScan);
  diagnosticsExportReportBtn?.addEventListener("click", handleExportReport);
  diagnosticsExportErrorsBtn?.addEventListener("click", handleExportErrors);
  diagnosticsExportCrashBtn?.addEventListener("click", handleExportCrash);
  diagnosticsCopySummaryBtn?.addEventListener("click", handleCopySummary);
  diagnosticsOpenLogFolderBtn?.addEventListener("click", handleOpenLogs);
  modalConflictDiagnosticsRetryBtn?.addEventListener("click", handleRefresh);
  modalConflictDiagnosticsDeepBtn?.addEventListener("click", handleDeepScan);
  diagnosticsSeverityFilter?.addEventListener("change", handleSeverityFilterChange);

  function handleSeverityFilterChange(event) {
    currentDiagnosticsSeverityValue = safeValue(event?.target?.value, "all").toLowerCase();
    if (currentDiagnosticsReport) {
      renderDiagnosticsErrorList(currentDiagnosticsReport, copy);
    }
  }

  async function loadDiagnosticsCopy() {
    return createDiagnosticsCopyMap({
      statusClean: await window.t("modals.mod_conflict_diagnostics.status.clean"),
      statusWarnings: await window.t("modals.mod_conflict_diagnostics.status.warnings"),
      statusIssuesFound: await window.t("modals.mod_conflict_diagnostics.status.issues_found"),
      statusNotEnoughData: await window.t("modals.mod_conflict_diagnostics.status.not_enough_data"),
      confidenceLow: await window.t("modals.mod_conflict_diagnostics.status.low"),
      confidencePossible: await window.t("modals.mod_conflict_diagnostics.status.possible"),
      confidenceLikely: await window.t("modals.mod_conflict_diagnostics.status.likely"),
      confidenceHigh: await window.t("modals.mod_conflict_diagnostics.status.high"),
      activeStateActive: await window.t("modals.mod_conflict_diagnostics.status.active"),
      activeStateNotActive: await window.t("modals.mod_conflict_diagnostics.status.not_active"),
      activeStateUnknown: await window.t("modals.mod_conflict_diagnostics.status.unknown"),
      availabilityFound: await window.t("modals.mod_conflict_diagnostics.status.found"),
      availabilityMissing: await window.t("modals.mod_conflict_diagnostics.status.missing"),
      severityInfo: await window.t("modals.mod_conflict_diagnostics.status.info"),
      severityWarning: await window.t("modals.mod_conflict_diagnostics.status.warning"),
      severityError: await window.t("modals.mod_conflict_diagnostics.status.error"),
      severityCritical: await window.t("modals.mod_conflict_diagnostics.status.critical"),
      contextGame: await window.t("modals.mod_conflict_diagnostics.context.game"),
      contextProfile: await window.t("modals.mod_conflict_diagnostics.context.profile"),
      contextSave: await window.t("modals.mod_conflict_diagnostics.context.save"),
      contextBasePath: await window.t("modals.mod_conflict_diagnostics.context.base_path"),
      sourceGameLog: await window.t("modals.mod_conflict_diagnostics.sources.game_log"),
      sourceGameCrash: await window.t("modals.mod_conflict_diagnostics.sources.game_crash"),
      sourceModFolder: await window.t("modals.mod_conflict_diagnostics.sources.mod_folder"),
      sourceIndexedMods: await window.t("modals.mod_conflict_diagnostics.sources.indexed_mods"),
      sourceExtractedErrors: await window.t("modals.mod_conflict_diagnostics.sources.extracted_errors"),
      sourceActiveMods: await window.t("modals.mod_conflict_diagnostics.sources.active_mods"),
      fieldPackage: await window.t("modals.mod_conflict_diagnostics.fields.package"),
      fieldFile: await window.t("modals.mod_conflict_diagnostics.fields.file"),
      fieldSource: await window.t("modals.mod_conflict_diagnostics.fields.source"),
      fieldPath: await window.t("modals.mod_conflict_diagnostics.fields.path"),
      fieldReadable: await window.t("modals.mod_conflict_diagnostics.fields.readable"),
      fieldManifest: await window.t("modals.mod_conflict_diagnostics.fields.manifest"),
      fieldCategories: await window.t("modals.mod_conflict_diagnostics.fields.categories"),
      fieldProfileInferred: await window.t("modals.mod_conflict_diagnostics.fields.profile_inferred"),
      fieldSaveInferred: await window.t("modals.mod_conflict_diagnostics.fields.save_inferred"),
      noSuspectedMods: await window.t("modals.mod_conflict_diagnostics.content.no_suspected_mods"),
      noMissingReferences: await window.t("modals.mod_conflict_diagnostics.content.no_missing_references"),
      noErrors: await window.t("modals.mod_conflict_diagnostics.content.no_errors"),
      noCrashContext: await window.t("modals.mod_conflict_diagnostics.content.no_crash_context"),
      noLogs: await window.t("modals.mod_conflict_diagnostics.content.no_logs"),
      noLimitations: await window.t("modals.mod_conflict_diagnostics.content.no_limitations"),
      removedSuspected: await window.t("modals.mod_conflict_diagnostics.content.removed_suspected"),
      noModAssigned: await window.t("modals.mod_conflict_diagnostics.content.no_mod_assigned"),
      filterAll: await window.t("modals.mod_conflict_diagnostics.errors.filter_all"),
      filterInfo: await window.t("modals.mod_conflict_diagnostics.errors.filter_info"),
      filterWarning: await window.t("modals.mod_conflict_diagnostics.errors.filter_warning"),
      filterError: await window.t("modals.mod_conflict_diagnostics.errors.filter_error"),
      filterCritical: await window.t("modals.mod_conflict_diagnostics.errors.filter_critical"),
      rawLineLabel: await window.t("modals.mod_conflict_diagnostics.errors.raw_line"),
      lastContextLabel: await window.t("modals.mod_conflict_diagnostics.errors.last_context"),
      logsTechnical: await window.t("modals.mod_conflict_diagnostics.exports.technical_log"),
      logsUser: await window.t("modals.mod_conflict_diagnostics.exports.user_log"),
      logsFolder: await window.t("modals.mod_conflict_diagnostics.exports.log_folder"),
      limitLabel: await window.t("modals.mod_conflict_diagnostics.content.limit_label"),
      protectedModsTitle: await window.t("modals.mod_conflict_diagnostics.content.protected_mods_title"),
      protectedModsHint: await window.t("modals.mod_conflict_diagnostics.content.protected_mods_hint"),
      analysisTimedOut: await window.t("toasts.diagnostics_analysis_timed_out"),
      deepScanWarning: await window.t("modals.mod_conflict_diagnostics.actions.deep_scan_warning"),
      errorBody: await window.t("modals.mod_conflict_diagnostics.error_body"),
    });
  }

  async function runAnalysis(command = "analyze_mod_conflict_diagnostics") {
    const runId = ++activeRun;
    if (isModAnalysisRunning) {
      await logDiagnosticsFrontendEvent("analysis_already_running", "Mod analysis already running", false);
      throw new Error("Mod analysis already running");
    }
    isModAnalysisRunning = true;
    currentDiagnosticsSeverityValue = "all";
    currentDiagnosticsReport = null;
    resetDiagnosticsModalPanels();
    setDiagnosticsActionState(true);
    if (modalConflictDiagnosticsErrorText) modalConflictDiagnosticsErrorText.textContent = "";
    setModalPillState(modalConflictDiagnosticsConfidence, "loading", copy.statusNotEnoughData);
    setModalPillState(modalConflictDiagnosticsHealth, "loading", await window.t("modals.mod_conflict_diagnostics.loading_title"));
    if (modalConflictDiagnosticsLoading) modalConflictDiagnosticsLoading.hidden = false;

    try {
      copy = await loadDiagnosticsCopy();
      if (isStaleRun(runId)) return;

      setModalPillState(modalConflictDiagnosticsConfidence, "loading", copy.statusNotEnoughData);
      setModalPillState(modalConflictDiagnosticsHealth, "loading", await window.t("modals.mod_conflict_diagnostics.loading_title"));
      await logDiagnosticsFrontendEvent("analysis_started", `Invoking ${command}`, false);

      const report = normalizeDiagnosticsReport(await window.invoke(command));
      if (isStaleRun(runId)) return;
      currentDiagnosticsReport = report;

      const hasVisibleData = Boolean(
        (report.sources?.game_log_found)
        || (report.sources?.game_crash_found)
        || (report.sources?.mod_folder_found)
        || (report.suspected_mods || []).length
        || (report.missing_references || []).length
        || (report.errors || []).length
        || (report.limitations || []).length
      );

      resetDiagnosticsModalPanels();
      if (!hasVisibleData) {
        if (modalConflictDiagnosticsEmpty) modalConflictDiagnosticsEmpty.hidden = false;
        setModalPillState(modalConflictDiagnosticsConfidence, "neutral", diagnosticsLabel(copy.status, "Not enough data"));
        setModalPillState(modalConflictDiagnosticsHealth, "neutral", diagnosticsLabel(copy.confidence, "Low"));
        await logDiagnosticsFrontendEvent("analysis_empty", "Analyzer returned no visible data", false);
        return;
      }

      renderDiagnosticsReport(report, copy);
      if (isStaleRun(runId)) return;
      if (report.sources?.analysis_timed_out) {
        window.showToast("toasts.diagnostics_analysis_timed_out", "warning");
      }

      if (modalConflictDiagnosticsContent) modalConflictDiagnosticsContent.hidden = false;
      setDiagnosticsActionState(false);
      if (diagnosticsOpenLogFolderBtn) {
        diagnosticsOpenLogFolderBtn.disabled = !safeValue(report.logs?.log_directory_path, "");
      }
      await logDiagnosticsFrontendEvent(
        "analysis_complete",
        `suspects=${report.suspected_mods.length} missing_refs=${report.missing_references.length} errors=${report.errors.length}`,
        false
      );
      if (window.logUserAction) window.logUserAction("mod_conflict_analyzer", "success");
    } catch (error) {
      if (safeValue(error?.message || error, "") !== "Mod analysis already running") {
        console.error("Diagnostics analysis failed:", error);
      }
      if (isStaleRun(runId)) return;
      const errorMessage = safeValue(error?.message || error, "Analyzer failed to load this data.");
      resetDiagnosticsModalPanels();
      if (modalConflictDiagnosticsError) modalConflictDiagnosticsError.hidden = false;
      if (modalConflictDiagnosticsErrorText) {
        modalConflictDiagnosticsErrorText.textContent = `${copy.errorBody} ${errorMessage}`.trim();
      }
      setModalPillState(modalConflictDiagnosticsConfidence, "error", copy.severityError);
      setModalPillState(modalConflictDiagnosticsHealth, "error", copy.statusIssuesFound);
      await logDiagnosticsFrontendEvent("analysis_failed", errorMessage, true);
      if (window.logUserAction) window.logUserAction("mod_conflict_analyzer", "error");
      window.showToast(
        errorMessage === "Mod analysis already running"
          ? errorMessage
          : "toasts.diagnostics_analysis_failed",
        errorMessage === "Mod analysis already running" ? "warning" : "error"
      );
    } finally {
      isModAnalysisRunning = false;
      if (!isStaleRun(runId)) {
        setDiagnosticsActionState(false);
      }
    }
  }

  async function handleDeepScan() {
    if (isModAnalysisRunning) return;
    if (!window.confirm(copy.deepScanWarning)) return;
    await runAnalysis("analyze_mod_conflict_diagnostics_deep");
  }

  setTimeout(() => {
    void runAnalysis();
  }, 0);
}

/* --------------------------------------------------------------
   PROFILE SHARING MODAL
-------------------------------------------------------------- */
export async function openProfileSharingModal(mode = "export", options = {}) {
  const activeMode = mode === "import" ? "import" : "export";
  const isImportMode = activeMode === "import";
  const openSurface = typeof options.openSurface === "function"
    ? options.openSurface
    : () => {
        modalProfileShare.style.display = "flex";
      };
  const closeSurface = typeof options.closeSurface === "function"
    ? options.closeSurface
    : () => {
        modalProfileShare.style.display = "none";
      };
  const resolveProfilePath = typeof options.resolveProfilePath === "function"
    ? options.resolveProfilePath
    : () => window.selectedProfilePath || resolveStoredProfileSharePath();
  const allowBackdropClose = options.allowBackdropClose ?? true;
  const allowEscape = options.allowEscape ?? true;
  const allowMissingProfilePath = options.allowMissingProfilePath ?? false;
  const onClose = typeof options.onClose === "function" ? options.onClose : null;

  if (!modalProfileShare) return;
  if (!allowMissingProfilePath && !isImportMode && !resolveProfilePath()) {
    window.showToast("toasts.profile_not_selected", "warning");
    return;
  }

  const copy = {
    readyTitle: await window.t("modals.profile_sharing.status_ready"),
    readyImportMessage: await window.t("modals.profile_sharing.status_ready_import"),
    readyExportMessage: await window.t("modals.profile_sharing.status_ready_export"),
    checkingMessage: await window.t("modals.profile_sharing.status_checking"),
    exportTitle: await window.t("modals.profile_sharing.status_exporting"),
    importTitle: await window.t("modals.profile_sharing.status_importing"),
    successTitle: await window.t("modals.profile_sharing.status_success"),
    errorTitle: await window.t("modals.profile_sharing.status_error"),
    importKicker: await window.t("modals.profile_sharing.kicker_import"),
    exportKicker: await window.t("modals.profile_sharing.kicker_export"),
    importModalTitle: await window.t("modals.profile_sharing.title_import"),
    exportModalTitle: await window.t("modals.profile_sharing.title_export"),
    importDescription: await window.t("modals.profile_sharing.description_import"),
    exportDescription: await window.t("modals.profile_sharing.description_export"),
    importModeHint: await window.t("modals.profile_sharing.mode_hint_import"),
    exportModeHint: await window.t("modals.profile_sharing.mode_hint_export"),
    importBrowseButton: await window.t("modals.profile_sharing.browse_import_button"),
    exportBrowseButton: await window.t("modals.profile_sharing.browse_export_button"),
    importPrimaryAction: await window.t("modals.profile_sharing.import_primary_action"),
    exportPrimaryAction: await window.t("modals.profile_sharing.export_primary_action"),
    importBusyLabel: await window.t("modals.profile_sharing.import_busy"),
    exportBusyLabel: await window.t("modals.profile_sharing.export_busy"),
    importHint: await window.t("modals.profile_sharing.import_hint"),
    exportHint: await window.t("modals.profile_sharing.export_hint"),
    importEmptyPath: await window.t("modals.profile_sharing.selected_path_empty_import"),
    exportEmptyPath: await window.t("modals.profile_sharing.selected_path_empty_export"),
    importTargetLabel: await window.t("modals.profile_sharing.import_target"),
    exportTargetLabel: await window.t("modals.profile_sharing.export_target"),
    selectedArchiveLabel: await window.t("modals.profile_sharing.selected_archive_label"),
    selectedExportDirLabel: await window.t("modals.profile_sharing.selected_export_dir_label"),
    previewProfileLabel: await window.t("modals.profile_sharing.preview_profile"),
    previewFilesLabel: await window.t("modals.profile_sharing.preview_files"),
    previewManifestLabel: await window.t("modals.profile_sharing.preview_manifest"),
    previewFinalProfileLabel: await window.t("modals.profile_sharing.preview_final_profile"),
    previewTargetFolderLabel: await window.t("modals.profile_sharing.preview_target_folder"),
    previewNameConflictLabel: await window.t("modals.profile_sharing.preview_name_conflict"),
    yesLabel: await window.t("modals.profile_sharing.preview_yes"),
    noLabel: await window.t("modals.profile_sharing.preview_no"),
  };

  let inspectTimer = null;
  let importReady = false;
  let isBusy = false;
  let context = null;
  let selectedImportArchivePath = "";
  let selectedExportDir = "";

  function currentReadyMessage() {
    return isImportMode ? copy.readyImportMessage : copy.readyExportMessage;
  }

  function currentSelectedPath() {
    return isImportMode ? selectedImportArchivePath : selectedExportDir;
  }

  function currentEmptyPathText() {
    return isImportMode ? copy.importEmptyPath : copy.exportEmptyPath;
  }

  function renderPreviewContent(content, isError = false) {
    const className = isError
      ? "share-preview-copy share-preview-copy--error"
      : "share-preview-copy";
    profileSharePreview.innerHTML = `<p class="${className}">${escapeHtml(content)}</p>`;
  }

  function renderStaticFields() {
    profileShareModeKicker.textContent = isImportMode ? copy.importKicker : copy.exportKicker;
    profileShareModalTitle.textContent = isImportMode ? copy.importModalTitle : copy.exportModalTitle;
    profileShareModalDescription.textContent = isImportMode ? copy.importDescription : copy.exportDescription;
    profileShareModeHint.textContent = isImportMode ? copy.importModeHint : copy.exportModeHint;
    profileShareWorkspaceTitle.textContent = isImportMode ? copy.importModalTitle : copy.exportModalTitle;
    profileShareTargetLabel.textContent = isImportMode ? copy.importTargetLabel : copy.exportTargetLabel;
    profileShareSelectedPathLabel.textContent = isImportMode ? copy.selectedArchiveLabel : copy.selectedExportDirLabel;
    profileSharePickerLabel.textContent = isImportMode ? copy.selectedArchiveLabel : copy.selectedExportDirLabel;
    profileShareBrowseButton.textContent = isImportMode ? copy.importBrowseButton : copy.exportBrowseButton;
    modalProfileSharePrimary.textContent = isImportMode ? copy.importPrimaryAction : copy.exportPrimaryAction;
    profileShareImportOptions.hidden = !isImportMode;

    const selectedPath = currentSelectedPath();
    const targetPath = isImportMode
      ? context?.importTargetDir || "-"
      : selectedExportDir || context?.defaultExportDir || "-";

    profileShareSourceName.textContent = safeValue(context?.profileName);
    profileShareArchiveName.textContent = safeValue(context?.defaultArchiveName);
    profileShareTargetPath.textContent = safeValue(targetPath);
    profileShareSelectedPath.textContent = safeValue(selectedPath, currentEmptyPathText());
    profileShareSelectionDisplay.textContent = safeValue(selectedPath, currentEmptyPathText());
  }

  function syncActionState() {
    const hasPathResolutionIssue = isImportMode ? !context?.canImport : !context?.canExport;
    const hasSelection = Boolean(currentSelectedPath());
    const canRunPrimary = isImportMode ? importReady : hasSelection;

    profileShareBrowseButton.disabled = isBusy || hasPathResolutionIssue;
    modalProfileSharePrimary.disabled = isBusy || hasPathResolutionIssue || !canRunPrimary;
    modalProfileShareClose.disabled = isBusy;
    profileShareImportName.disabled = isBusy || !isImportMode;
  }

  async function inspectArchive() {
    if (!selectedImportArchivePath) {
      importReady = false;
      renderPreviewContent(copy.importHint);
      setProfileShareStatus("neutral", copy.readyTitle, currentReadyMessage());
      syncActionState();
      return;
    }

    setProfileShareStatus("loading", copy.readyTitle, copy.checkingMessage, selectedImportArchivePath);
    importReady = false;
    syncActionState();

    try {
      const preview = await window.invoke("inspect_shared_profile_archive", {
        archivePath: selectedImportArchivePath,
        profileNameOverride: profileShareImportName.value.trim() || null,
      });
      if (!profileShareImportName.value.trim()) {
        profileShareImportName.placeholder = preview.suggestedProfileName;
      }

      profileShareTargetPath.textContent = safeValue(preview.targetProfilePath);
      profileSharePreview.innerHTML = `
        <div class="share-preview-grid">
          <div class="share-preview-item">
            <span>${escapeHtml(copy.previewProfileLabel)}</span>
            <strong>${escapeHtml(preview.detectedProfileName)}</strong>
          </div>
          <div class="share-preview-item">
            <span>${escapeHtml(copy.previewFinalProfileLabel)}</span>
            <strong>${escapeHtml(preview.finalProfileName)}</strong>
          </div>
          <div class="share-preview-item">
            <span>${escapeHtml(copy.previewFilesLabel)}</span>
            <strong>${escapeHtml(preview.fileCount)}</strong>
          </div>
          <div class="share-preview-item">
            <span>${escapeHtml(copy.previewManifestLabel)}</span>
            <strong>${escapeHtml(preview.hasManifest ? copy.yesLabel : copy.noLabel)}</strong>
          </div>
          <div class="share-preview-item">
            <span>${escapeHtml(copy.previewNameConflictLabel)}</span>
            <strong>${escapeHtml(preview.profileNameConflict ? copy.yesLabel : copy.noLabel)}</strong>
          </div>
          <div class="share-preview-item share-preview-item--stack">
            <span>${escapeHtml(copy.previewTargetFolderLabel)}</span>
            <strong class="detail-card-value--mono">${escapeHtml(preview.targetProfilePath)}</strong>
          </div>
        </div>
      `;
      importReady = true;
      if (preview.profileNameConflict) {
        setProfileShareStatus(
          "warning",
          copy.readyTitle,
          await window.t("modals.profile_sharing.import_conflict_message", {
            profileName: preview.finalProfileName,
          }),
          preview.targetProfilePath
        );
      } else {
        setProfileShareStatus("success", copy.readyTitle, currentReadyMessage(), preview.targetProfilePath);
      }
    } catch (error) {
      const errorMessage = error?.message || String(error);
      console.error("Profile archive inspection failed:", error);
      renderPreviewContent(errorMessage, true);
      setProfileShareStatus("error", copy.errorTitle, errorMessage, selectedImportArchivePath);
    } finally {
      syncActionState();
    }
  }

  function scheduleInspect() {
    clearTimeout(inspectTimer);
    inspectTimer = setTimeout(() => {
      void inspectArchive();
    }, 220);
  }

  async function handleBrowse() {
    if (isBusy) return;

    try {
      if (isImportMode) {
        const archivePath = await window.invoke("pick_shared_profile_import_archive");
        if (!archivePath) return;
        selectedImportArchivePath = archivePath;
        renderStaticFields();
        await inspectArchive();
        return;
      }

      const exportDir = await window.invoke("pick_shared_profile_export_directory");
      if (!exportDir) return;
      selectedExportDir = exportDir;
      renderStaticFields();
      setProfileShareStatus("neutral", copy.readyTitle, currentReadyMessage(), exportDir);
      syncActionState();
    } catch (error) {
      const errorMessage = error?.message || String(error);
      console.error("Profile share picker failed:", error);
      setProfileShareStatus("error", copy.errorTitle, errorMessage);
      window.showToast("toasts.profile_share_action_failed", { error: errorMessage }, "error");
    }
  }

  async function handlePrimaryAction() {
    if (isBusy) return;

    if (isImportMode) {
      if (!selectedImportArchivePath || !importReady) return;

      isBusy = true;
      modalProfileSharePrimary.textContent = copy.importBusyLabel;
      syncActionState();
      setProfileShareStatus("loading", copy.importTitle, await window.t("modals.profile_sharing.import_progress"));
      if (window.logUserAction) window.logUserAction("profile_share_import", "start");

      try {
        const result = await window.invoke("import_shared_profile", {
          archivePath: selectedImportArchivePath,
          profileNameOverride: profileShareImportName.value.trim() || null,
        });
        profileShareImportName.value = result.profileName;
        profileShareTargetPath.textContent = safeValue(result.profilePath);
        setProfileShareStatus(
          "success",
          copy.successTitle,
          await window.t("modals.profile_sharing.import_done", {
            profileName: result.profileName,
          }),
          result.profilePath
        );
        window.showToast("toasts.profile_share_import_success", { profileName: result.profileName }, "success");
        document.querySelector("#refreshBtn")?.click();
        if (window.logUserAction) window.logUserAction("profile_share_import", "success");
      } catch (error) {
        const errorMessage = error?.message || String(error);
        console.error("Profile import failed:", error);
        setProfileShareStatus("error", copy.errorTitle, errorMessage);
        window.showToast("toasts.profile_share_action_failed", { error: errorMessage }, "error");
        if (window.logUserAction) window.logUserAction("profile_share_import", "error");
      } finally {
        isBusy = false;
        modalProfileSharePrimary.textContent = copy.importPrimaryAction;
        syncActionState();
      }
      return;
    }

    const activeProfilePath = resolveProfilePath();

    if (!activeProfilePath) {
      window.showToast("toasts.profile_not_selected", "warning");
      return;
    }

    if (!selectedExportDir) {
      await handleBrowse();
      if (!selectedExportDir) return;
    }

    isBusy = true;
    modalProfileSharePrimary.textContent = copy.exportBusyLabel;
    syncActionState();
    setProfileShareStatus("loading", copy.exportTitle, await window.t("modals.profile_sharing.export_progress"));
    if (window.logUserAction) window.logUserAction("profile_share_export", "start");

    try {
      const result = await window.invoke("export_shared_profile", {
        profilePath: activeProfilePath,
        exportDirOverride: selectedExportDir,
      });
      profileShareArchiveName.textContent = result.archiveName;
      profileShareTargetPath.textContent = safeValue(result.exportDir);
      profileShareSelectedPath.textContent = safeValue(result.exportDir);
      profileShareSelectionDisplay.textContent = safeValue(result.exportDir);
      setProfileShareStatus(
        "success",
        copy.successTitle,
        await window.t("modals.profile_sharing.export_done", {
          fileCount: result.exportedFiles,
        }),
        result.archivePath
      );
      renderPreviewContent(copy.exportHint);
      window.showToast("toasts.profile_share_export_success", { profileName: result.profileName }, "success");
      if (window.logUserAction) window.logUserAction("profile_share_export", "success");
    } catch (error) {
      const errorMessage = error?.message || String(error);
      console.error("Profile export failed:", error);
      setProfileShareStatus("error", copy.errorTitle, errorMessage);
      window.showToast("toasts.profile_share_action_failed", { error: errorMessage }, "error");
      if (window.logUserAction) window.logUserAction("profile_share_export", "error");
    } finally {
      isBusy = false;
      modalProfileSharePrimary.textContent = copy.exportPrimaryAction;
      syncActionState();
    }
  }

  function cleanup() {
    if (isBusy) return;
    clearTimeout(inspectTimer);
    profileShareBrowseButton.removeEventListener("click", handleBrowse);
    modalProfileSharePrimary.removeEventListener("click", handlePrimaryAction);
    modalProfileShareClose.removeEventListener("click", cleanup);
    if (allowBackdropClose) {
      modalProfileShare.removeEventListener("click", handleBackdropClick);
    }
    profileShareImportName.removeEventListener("input", scheduleInspect);
    if (allowEscape) {
      document.removeEventListener("keydown", handleEscape);
    }
    closeSurface();
    onClose?.();
  }

  function handleBackdropClick(event) {
    if (!allowBackdropClose) return;
    if (event.target === modalProfileShare) {
      cleanup();
    }
  }

  function handleEscape(event) {
    if (!allowEscape) return;
    if (event.key === "Escape") {
      cleanup();
    }
  }

  profileShareImportName.value = "";
  profileShareImportName.placeholder = await window.t("modals.profile_sharing.import_name_placeholder");
  renderPreviewContent(isImportMode ? copy.importHint : copy.exportHint);
  setProfileShareStatus("neutral", copy.readyTitle, currentReadyMessage());
  openSurface();

  try {
    const activeProfilePath = isImportMode ? null : resolveProfilePath();
    context = await window.invoke("get_profile_share_context", {
      profilePath: activeProfilePath,
    });
    if (!isImportMode) {
      selectedExportDir = context?.defaultExportDir || "";
    }
    renderStaticFields();

    const pathResolutionMessage = context?.pathResolutionError || "";
    if ((isImportMode && !context?.canImport) || (!isImportMode && !context?.canExport)) {
      setProfileShareStatus("error", copy.errorTitle, pathResolutionMessage || currentReadyMessage());
    } else {
      setProfileShareStatus("neutral", copy.readyTitle, currentReadyMessage(), currentSelectedPath());
    }
  } catch (error) {
    const errorMessage = error?.message || String(error);
    console.error("Profile share context failed:", error);
    setProfileShareStatus("error", copy.errorTitle, errorMessage);
  }

  syncActionState();
  profileShareBrowseButton.addEventListener("click", handleBrowse);
  modalProfileSharePrimary.addEventListener("click", handlePrimaryAction);
  modalProfileShareClose.addEventListener("click", cleanup);
  if (allowBackdropClose) {
    modalProfileShare.addEventListener("click", handleBackdropClick);
  }
  profileShareImportName.addEventListener("input", scheduleInspect);
  if (allowEscape) {
    document.addEventListener("keydown", handleEscape);
  }
}

export function openProfileSharingPage(mode = "export") {
  const activeMode = mode === "import" ? "import" : "export";
  localStorage.setItem(
    "ets2_profile_share_profile_path",
    window.selectedProfilePath ? String(window.selectedProfilePath) : ""
  );
  window.location.href = `/pages/profile-sharing/index.html?mode=${encodeURIComponent(activeMode)}`;
}

export function openModConflictDiagnosticsPage() {
  window.showToast?.("toasts.coming_soon", "warning");
}

export function openModProfileManagerPage() {
  console.info("[trace] START open_mod_manager");
  void openModProfileManagerModal();
}

const modProfileManagerState = {
  presets: [],
  checks: new Map(),
  checkErrors: new Map(),
  activations: new Map(),
  loading: false,
  checkingPresetId: null,
  activatingPresetId: null,
  progressLog: [],
};

function hasActiveSaveSelected() {
  return Boolean(window.selectedProfilePath && window.selectedSavePath);
}

function shortPathLabel(path) {
  const value = String(path || "").replace(/\\/g, "/");
  if (!value) return "-";
  const segments = value.split("/").filter(Boolean);
  return segments.slice(-2).join(" / ") || value;
}

function normalizeSandboxPresets(payload) {
  return Array.isArray(payload) ? payload : [];
}

function getSandboxPresetCheck(presetId) {
  return modProfileManagerState.checks.get(presetId) || null;
}

function getSandboxPresetCheckError(presetId) {
  return modProfileManagerState.checkErrors.get(presetId) || "";
}

function getSandboxPresetActivation(presetId) {
  return modProfileManagerState.activations.get(presetId) || null;
}

function setModProfileBusy(busy) {
  modProfileManagerState.loading = Boolean(busy);
  if (modSandboxReloadBtn) modSandboxReloadBtn.disabled = Boolean(busy);
  if (modSteamConsoleBtn) modSteamConsoleBtn.disabled = Boolean(busy);
}

async function setModProfileStatus(key, state = "neutral", params = {}) {
  if (!modProfileManagerStatusPill) return;
  modProfileManagerStatusPill.dataset.state = state;
  modProfileManagerStatusPill.textContent = await window.t(key, params);
}

async function updateModProfileContextLabels() {
  if (modActiveProfileName) {
    modActiveProfileName.textContent = window.selectedProfilePath
      ? shortPathLabel(window.selectedProfilePath)
      : await window.t("editor.no_profile");
  }
  if (modActiveSaveName) {
    modActiveSaveName.textContent = window.selectedSavePath
      ? shortPathLabel(window.selectedSavePath)
      : await window.t("editor.no_save");
  }
  if (modProfileManagerProfilePill) {
    const hasProfile = Boolean(window.selectedProfilePath);
    const hasSave = Boolean(window.selectedSavePath);
    modProfileManagerProfilePill.dataset.state = hasProfile && hasSave ? "success" : "warning";
    modProfileManagerProfilePill.textContent = hasProfile && hasSave
      ? await window.t("modals.mod_profile_manager.sandbox.status.active_save_ready")
      : await window.t("modals.mod_profile_manager.sandbox.status.no_active_save");
  }
}

async function translateSandboxProgressEntry(entry) {
  const mapping = {
    "Preset geladen": "modals.mod_profile_manager.sandbox.progress.preset_loaded",
    "Mods geprüft": "modals.mod_profile_manager.sandbox.progress.mods_checked",
    "Profil geöffnet": "modals.mod_profile_manager.sandbox.progress.profile_opened",
    "actived_mods gelesen": "modals.mod_profile_manager.sandbox.progress.active_mods_read",
    "Backup erstellt": "modals.mod_profile_manager.sandbox.progress.backup_created",
    "actived_mods geschrieben": "modals.mod_profile_manager.sandbox.progress.active_mods_written",
    "Follow-up Check erfolgreich": "modals.mod_profile_manager.sandbox.progress.follow_up_successful",
  };
  const key = mapping[entry];
  return key ? window.t(key) : Promise.resolve(String(entry || ""));
}

async function renderSandboxProgressList(entries = modProfileManagerState.progressLog) {
  if (!modSandboxProgressList) return;
  const items = Array.isArray(entries) ? entries.filter(Boolean) : [];
  if (!items.length) {
    modSandboxProgressList.innerHTML = `
      <div class="sandbox-progress-item is-muted">
        ${escapeHtml(await window.t("modals.mod_profile_manager.sandbox.hints.progress"))}
      </div>
    `;
    return;
  }

  const translated = await Promise.all(items.map((entry) => translateSandboxProgressEntry(entry)));
  modSandboxProgressList.innerHTML = translated.map((entry) => `
    <div class="sandbox-progress-item">${escapeHtml(entry)}</div>
  `).join("");
}

async function showSandboxPresetPopup(type, title, message) {
  window.showToast?.(`${title}: ${message}`, type);
}

async function openSandboxModWorkshopPage(steamId) {
  console.log("[SandboxPreset] open workshop page:", steamId);
  await window.invoke("open_sandbox_mod_workshop_page", { steamId });
}

async function openSandboxModInSteam(steamId) {
  console.log("[SandboxPreset] open mod in steam:", steamId);
  await window.invoke("open_sandbox_mod_in_steam", { steamId });
}

function normalizeSandboxCommandError(error) {
  if (typeof error === "string") {
    return error.trim();
  }
  if (error && typeof error.message === "string") {
    return error.message.trim();
  }
  return String(error || "").trim();
}

async function sandboxErrorTitle(errorCode) {
  switch (String(errorCode || "")) {
    case "mod_not_found":
      return window.t("modals.mod_profile_manager.sandbox.popup.mod_missing_title");
    case "save_write_failed":
      return window.t("modals.mod_profile_manager.sandbox.popup.save_failed_title");
    case "verification_failed":
    case "save_reread_failed":
      return window.t("modals.mod_profile_manager.sandbox.popup.verification_failed_title");
    default:
      return window.t("modals.mod_profile_manager.sandbox.popup.error_title");
  }
}

async function renderSandboxInlineMessage(state, title, message) {
  if (!modApplySandboxResult) return;
  modApplySandboxResult.hidden = false;
  modApplySandboxResult.dataset.state = state;
  modApplySandboxResult.innerHTML = `
    <div class="mod-apply-result-head">
      <strong>${escapeHtml(title)}</strong>
    </div>
    <p>${escapeHtml(message)}</p>
  `;
}

async function sandboxErrorMessage(result) {
  switch (String(result?.error_code || "")) {
    case "mod_not_found":
      return String(result?.message || "");
    case "save_write_failed":
      return await window.t("modals.mod_profile_manager.sandbox.popup.save_failed_message");
    case "verification_failed":
    case "save_reread_failed":
      return await window.t("modals.mod_profile_manager.sandbox.popup.verification_failed_message");
    case "no_active_save":
      return await window.t("modals.mod_profile_manager.sandbox.popup.no_active_save_message");
    case "actived_mods_missing":
      return await window.t("modals.mod_profile_manager.sandbox.popup.active_mods_missing_message");
    default:
      return String(result?.message || "");
  }
}

function sandboxCheckMessage(result) {
  if (result && typeof result.message === "string" && result.message.trim()) {
    return result.message.trim();
  }
  return "";
}

async function renderSandboxResult(result) {
  if (!modApplySandboxResult) return;
  if (!result) {
    modApplySandboxResult.hidden = true;
    modApplySandboxResult.innerHTML = "";
    return;
  }

  const isSuccess = Boolean(result.success);
  const title = isSuccess
    ? await window.t("modals.mod_profile_manager.sandbox.popup.success_title")
    : await sandboxErrorTitle(result.error_code);
  const message = isSuccess
    ? await window.t("modals.mod_profile_manager.sandbox.popup.success_message", { title: result.title || "" })
    : await sandboxErrorMessage(result);
  const backupLabel = await window.t("modals.mod_profile_manager.sandbox.apply_result.backup_created");
  const writtenLabel = await window.t("modals.mod_profile_manager.sandbox.apply_result.applied_mods", {
    count: Array.isArray(result.written_mods) ? result.written_mods.length : 0,
  });
  const verifiedLabel = await window.t("modals.mod_profile_manager.sandbox.fields.verified_mods");
  const verifiedMods = Array.isArray(result.verified_mods) && result.verified_mods.length
    ? result.verified_mods.join(", ")
    : "-";

  modApplySandboxResult.hidden = false;
  modApplySandboxResult.dataset.state = isSuccess ? "success" : "error";
  modApplySandboxResult.innerHTML = `
    <div class="mod-apply-result-head">
      <strong>${escapeHtml(title)}</strong>
    </div>
    <p>${escapeHtml(message)}</p>
    <div class="mod-apply-result-grid">
      <span>${escapeHtml(writtenLabel)}</span>
      <span>${escapeHtml(backupLabel)}: ${escapeHtml(result.backup_path || "-")}</span>
      <span>${escapeHtml(verifiedLabel)}: ${escapeHtml(verifiedMods)}</span>
    </div>
  `;
}

async function sandboxPresetStatusPresentation(preset) {
  if (modProfileManagerState.activatingPresetId === preset.id) {
    return {
      label: await window.t("modals.mod_profile_manager.sandbox.preset_status.activating"),
      state: "warning",
    };
  }

  const activation = getSandboxPresetActivation(preset.id);
  if (activation?.success) {
    return {
      label: await window.t("modals.mod_profile_manager.sandbox.preset_status.activated"),
      state: "success",
    };
  }
  if (activation && activation.success === false) {
    return {
      label: await window.t("modals.mod_profile_manager.sandbox.preset_status.error"),
      state: "error",
    };
  }

  const checkError = getSandboxPresetCheckError(preset.id);
  if (checkError) {
    return {
      label: await window.t("modals.mod_profile_manager.sandbox.preset_status.error"),
      state: "error",
    };
  }

  const check = getSandboxPresetCheck(preset.id);
  if (!check) {
    return {
      label: await window.t("modals.mod_profile_manager.sandbox.preset_status.not_checked"),
      state: "neutral",
    };
  }
  if (check.ready) {
    return {
      label: await window.t("modals.mod_profile_manager.sandbox.preset_status.ready"),
      state: "success",
    };
  }
  return {
    label: await window.t("modals.mod_profile_manager.sandbox.preset_status.mod_missing"),
    state: "warning",
  };
}

async function renderSandboxPresetCards() {
  if (!modSandboxPresetList || !modSandboxEmpty) return;

  const presets = modProfileManagerState.presets;
  if (modSandboxCount) modSandboxCount.textContent = String(presets.length);
  modSandboxEmpty.hidden = presets.length > 0;

  if (!presets.length) {
    modSandboxPresetList.innerHTML = "";
    return;
  }

  const checkLabel = await window.t("modals.mod_profile_manager.sandbox.actions.check_mods");
  const activateLabel = await window.t("modals.mod_profile_manager.sandbox.actions.activate_preset");
  const steamIdLabel = await window.t("modals.mod_profile_manager.sandbox.fields.steam_id");
  const modNameLabel = await window.t("modals.mod_profile_manager.sandbox.fields.mod_name");
  const loadOrderLabel = await window.t("modals.mod_profile_manager.sandbox.fields.load_order");
  const pathLabel = await window.t("modals.mod_profile_manager.sandbox.fields.local_path");
  const statusLabel = await window.t("modals.mod_profile_manager.sandbox.fields.status");
  const foundLabel = await window.t("modals.mod_profile_manager.status.found");
  const missingLabel = await window.t("modals.mod_profile_manager.status.missing");
  const notCheckedLabel = await window.t("modals.mod_profile_manager.sandbox.preset_status.not_checked");
  const openWorkshopLabel = await window.t("modals.mod_profile_manager.sandbox.actions.open_workshop");
  const openInSteamLabel = await window.t("modals.mod_profile_manager.sandbox.actions.open_in_steam");
  const missingHintLabel = await window.t("modals.mod_profile_manager.sandbox.hints.missing_install");

  const cards = await Promise.all(presets.map(async (preset) => {
    const statusPresentation = await sandboxPresetStatusPresentation(preset);
    const check = getSandboxPresetCheck(preset.id);
    const checkError = getSandboxPresetCheckError(preset.id);
    const activation = getSandboxPresetActivation(preset.id);
    const checkStatusMap = new Map(
      [...(check?.all_mods || []), ...(check?.found_mods || []), ...(check?.missing_mods || [])].map((entry) => [String(entry.steam_id), entry])
    );
    const canActivate = Boolean(check?.ready) && hasActiveSaveSelected() && !modProfileManagerState.loading
      && modProfileManagerState.checkingPresetId !== preset.id
      && modProfileManagerState.activatingPresetId !== preset.id;
    const isChecking = modProfileManagerState.checkingPresetId === preset.id;
    const isActivating = modProfileManagerState.activatingPresetId === preset.id;

    const modsMarkup = preset.mods.map((presetMod) => {
      const status = checkStatusMap.get(String(presetMod.steam_id));
      const state = !status
        ? "neutral"
        : status.found
          ? "found"
          : "missing";
      const statusText = !status
        ? notCheckedLabel
        : status.found
          ? foundLabel
          : missingLabel;
      const showMissingActions = Boolean(status) && !status.found;
      const resolvedWorkshopUrl = status?.workshop_url || presetMod.workshop_url || "";
      const resolvedSteamProtocolUrl = status?.steam_protocol_url || presetMod.steam_protocol_url || "";
      return `
        <article class="sandbox-preset-mod-row">
          <div class="sandbox-preset-mod-grid">
            <span><strong>${escapeHtml(modNameLabel)}:</strong> ${escapeHtml(status?.display_name || presetMod.display_name || "-")}</span>
            <span><strong>${escapeHtml(steamIdLabel)}:</strong> ${escapeHtml(String(presetMod.steam_id || "-"))}</span>
            <span><strong>${escapeHtml(loadOrderLabel)}:</strong> ${escapeHtml(String(status?.load_order ?? presetMod.load_order ?? 0))}</span>
            <span><strong>${escapeHtml(statusLabel)}:</strong> <span class="mod-status-badge" data-state="${escapeHtml(state)}">${escapeHtml(statusText)}</span></span>
            <span><strong>${escapeHtml(pathLabel)}:</strong> ${escapeHtml(status?.local_path || "-")}</span>
            ${showMissingActions ? `
              <div class="sandbox-preset-mod-actions">
                <button
                  class="secondary-action"
                  type="button"
                  data-sandbox-action="open-workshop"
                  data-sandbox-steam-id="${escapeHtml(String(presetMod.steam_id || ""))}"
                  ${resolvedWorkshopUrl ? "" : "disabled"}>
                  ${escapeHtml(openWorkshopLabel)}
                </button>
                <button
                  class="secondary-action"
                  type="button"
                  data-sandbox-action="open-steam"
                  data-sandbox-steam-id="${escapeHtml(String(presetMod.steam_id || ""))}"
                  ${resolvedSteamProtocolUrl ? "" : "disabled"}>
                  ${escapeHtml(openInSteamLabel)}
                </button>
              </div>
              <p class="sandbox-preset-mod-note">${escapeHtml(missingHintLabel)}</p>
            ` : ""}
          </div>
        </article>
      `;
    }).join("");

    let footerMessage = await window.t("modals.mod_profile_manager.sandbox.hints.not_checked");
    if (activation?.success === false) {
      footerMessage = await sandboxErrorMessage(activation);
    } else if (checkError) {
      footerMessage = checkError;
    } else if (check && !check.ready) {
      footerMessage = sandboxCheckMessage(check) || await window.t("modals.mod_profile_manager.sandbox.popup.mod_missing_message");
    } else if (check?.ready) {
      footerMessage = sandboxCheckMessage(check) || await window.t("modals.mod_profile_manager.sandbox.hints.ready");
    }

    return `
      <article class="sandbox-preset-card" data-preset-id="${escapeHtml(preset.id)}">
        <div class="mod-profile-panel-head">
          <div>
            <h3>${escapeHtml(preset.title || preset.id)}</h3>
            <p>${escapeHtml(preset.description || "-")}</p>
          </div>
          <span class="mod-status-badge" data-state="${escapeHtml(statusPresentation.state)}">${escapeHtml(statusPresentation.label)}</span>
        </div>
        <div class="sandbox-preset-actions">
          <button
            class="secondary-action"
            type="button"
            data-sandbox-action="check-mods"
            data-sandbox-preset-id="${escapeHtml(preset.id)}"
            ${isChecking || modProfileManagerState.loading ? "disabled" : ""}>
            ${escapeHtml(checkLabel)}
          </button>
          <button
            class="apply"
            type="button"
            data-sandbox-action="activate-preset"
            data-sandbox-preset-id="${escapeHtml(preset.id)}"
            ${canActivate ? "" : "disabled"}>
            ${escapeHtml(activateLabel)}
          </button>
        </div>
        <div class="sandbox-preset-mod-list">${modsMarkup}</div>
        <p class="sandbox-preset-hint">${escapeHtml(isActivating ? await window.t("modals.mod_profile_manager.sandbox.hints.activating") : footerMessage)}</p>
      </article>
    `;
  }));
  modSandboxPresetList.innerHTML = cards.join("");
  bindSandboxPresetButtonHandlers();
}

export async function openModProfileManagerModal() {
  if (!modalModProfileManager) return;

  modalModProfileManager.style.display = "flex";
  await updateModProfileContextLabels();
  await setModProfileStatus("modals.mod_profile_manager.status.ready", "neutral");
  await renderSandboxResult(null);
  await renderSandboxProgressList([]);
  await loadSandboxPresetsIntoModal();
}

function closeModProfileManagerModal() {
  if (!modalModProfileManager) return;
  modalModProfileManager.style.display = "none";
}

function bindSandboxPresetButtonHandlers() {
  if (!modSandboxPresetList) {
    console.warn("[SandboxPreset] modSandboxPresetList not found. Sandbox button handlers were not bound.");
    return;
  }

  const actionButtons = modSandboxPresetList.querySelectorAll("[data-sandbox-action]");
  actionButtons.forEach((button) => {
    if (button.dataset.sandboxBound === "1") return;
    button.dataset.sandboxBound = "1";

    button.addEventListener("click", async (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (typeof event.stopImmediatePropagation === "function") {
        event.stopImmediatePropagation();
      }

      const action = button.dataset.sandboxAction || "";
      const presetId = button.dataset.sandboxPresetId || "";
      const steamId = button.dataset.sandboxSteamId || "";

      console.log("[SandboxPreset] button clicked", {
        action,
        presetId,
        steamId,
      });

      if (action === "check-mods") {
        console.debug("[SandboxPreset] check button clicked", { selectedPresetId: presetId });
        await checkSandboxPresetMods(presetId);
        return;
      }

      if (action === "activate-preset") {
        console.debug("[SandboxPreset] activate button clicked", { selectedPresetId: presetId });
        await activateSandboxPreset(presetId);
        return;
      }

      if (action === "open-workshop") {
        try {
          console.debug("[SandboxPreset] open workshop clicked", { steamId });
          await openSandboxModWorkshopPage(steamId);
        } catch (error) {
          console.error("Open sandbox workshop page failed:", error);
          await showSandboxPresetPopup(
            "error",
            await window.t("modals.mod_profile_manager.sandbox.popup.error_title"),
            normalizeSandboxCommandError(error)
          );
        }
        return;
      }

      if (action === "open-steam") {
        try {
          console.debug("[SandboxPreset] open in Steam clicked", { steamId });
          await openSandboxModInSteam(steamId);
        } catch (error) {
          console.error("Open sandbox mod in Steam failed:", error);
          await showSandboxPresetPopup(
            "error",
            await window.t("modals.mod_profile_manager.sandbox.popup.error_title"),
            normalizeSandboxCommandError(error)
          );
        }
      }
    });
  });
}

export async function loadSandboxPresetsIntoModal() {
  setModProfileBusy(true);
  modProfileManagerState.checks.clear();
  modProfileManagerState.checkErrors.clear();
  modProfileManagerState.activations.clear();
  modProfileManagerState.progressLog = [];
  modProfileManagerState.checkingPresetId = null;
  modProfileManagerState.activatingPresetId = null;
  try {
    modProfileManagerState.presets = normalizeSandboxPresets(
      await window.invoke("load_sandbox_mod_presets")
    );
    await renderSandboxPresetCards();
    await renderSandboxProgressList([]);
    await setModProfileStatus("modals.mod_profile_manager.status.ready", "success");
  } catch (error) {
    console.error("Sandbox preset load failed:", error);
    modProfileManagerState.presets = [];
    if (modSandboxPresetList) modSandboxPresetList.innerHTML = "";
    if (modSandboxEmpty) modSandboxEmpty.hidden = false;
    await renderSandboxProgressList([]);
    await renderSandboxResult(null);
    await setModProfileStatus("modals.mod_profile_manager.sandbox.status.load_failed", "error");
    window.showToast?.("toasts.mod_sandbox_load_failed", "error");
  } finally {
    setModProfileBusy(false);
  }
}

async function checkSandboxPresetMods(presetId) {
  console.log("[SandboxPreset] checkSandboxPresetMods called", {
    presetId,
    hasInvoke: typeof window.invoke === "function",
  });
  modProfileManagerState.checkingPresetId = presetId;
  modProfileManagerState.activatingPresetId = null;
  modProfileManagerState.checkErrors.delete(presetId);
  await renderSandboxProgressList(["Preset geladen"]);
  await setModProfileStatus("modals.mod_profile_manager.sandbox.status.checking", "warning");
  await renderSandboxResult(null);
  await renderSandboxPresetCards();

  try {
    console.log("[SandboxPreset] invoking check_sandbox_preset_mods", { presetId });
    const result = await window.invoke("check_sandbox_preset_mods", { presetId });
    console.log("[SandboxPreset] check result:", result);
    console.log("[SandboxPreset] ready:", Boolean(result?.ready));
    modProfileManagerState.checks.set(presetId, result);
    modProfileManagerState.checkErrors.delete(presetId);
    modProfileManagerState.activations.delete(presetId);
    modProfileManagerState.progressLog = Array.isArray(result.progress_log) ? result.progress_log : [];
    await renderSandboxProgressList(modProfileManagerState.progressLog);
    await renderSandboxPresetCards();
    if (result.ready) {
      await setModProfileStatus("modals.mod_profile_manager.sandbox.status.ready", "success");
      await renderSandboxInlineMessage(
        "success",
        await window.t("modals.mod_profile_manager.sandbox.popup.check_success_title"),
        sandboxCheckMessage(result) || await window.t("modals.mod_profile_manager.sandbox.popup.check_success_message", { title: result.title || "" })
      );
      await showSandboxPresetPopup(
        "success",
        await window.t("modals.mod_profile_manager.sandbox.popup.check_success_title"),
        sandboxCheckMessage(result) || await window.t("modals.mod_profile_manager.sandbox.popup.check_success_message", { title: result.title || "" })
      );
    } else {
      await setModProfileStatus("modals.mod_profile_manager.sandbox.status.mod_missing", "warning");
      await renderSandboxInlineMessage(
        "error",
        await window.t("modals.mod_profile_manager.sandbox.popup.mod_missing_title"),
        sandboxCheckMessage(result) || await window.t("modals.mod_profile_manager.sandbox.popup.mod_missing_message")
      );
      await showSandboxPresetPopup(
        "warning",
        await window.t("modals.mod_profile_manager.sandbox.popup.mod_missing_title"),
        sandboxCheckMessage(result) || await window.t("modals.mod_profile_manager.sandbox.popup.mod_missing_message")
      );
    }
  } catch (error) {
    console.error("[SandboxPreset] check_sandbox_preset_mods failed", error);
    const errorMessage = normalizeSandboxCommandError(error)
      || await window.t("modals.mod_profile_manager.sandbox.popup.check_failed_message");
    modProfileManagerState.checks.delete(presetId);
    modProfileManagerState.checkErrors.set(presetId, errorMessage);
    modProfileManagerState.activations.delete(presetId);
    modProfileManagerState.progressLog = ["Preset geladen", errorMessage];
    await renderSandboxProgressList(modProfileManagerState.progressLog);
    await renderSandboxInlineMessage(
      "error",
      await window.t("modals.mod_profile_manager.sandbox.popup.error_title"),
      errorMessage
    );
    await setModProfileStatus("modals.mod_profile_manager.sandbox.status.check_failed", "error");
    await showSandboxPresetPopup(
      "error",
      await window.t("modals.mod_profile_manager.sandbox.popup.error_title"),
      errorMessage
    );
  } finally {
    modProfileManagerState.checkingPresetId = null;
    await renderSandboxPresetCards();
  }
}

async function activateSandboxPreset(presetId) {
  if (!hasActiveSaveSelected()) {
    await setModProfileStatus("modals.mod_profile_manager.sandbox.status.no_active_save", "warning");
    await showSandboxPresetPopup(
      "warning",
      await window.t("modals.mod_profile_manager.sandbox.popup.error_title"),
      await window.t("modals.mod_profile_manager.sandbox.popup.no_active_save_message")
    );
    return;
  }

  modProfileManagerState.activatingPresetId = presetId;
  await renderSandboxProgressList(["Preset geladen", "Mods geprüft"]);
  await setModProfileStatus("modals.mod_profile_manager.sandbox.preset_status.activating", "warning");
  await renderSandboxResult(null);
  await renderSandboxPresetCards();

  try {
    const result = await window.invoke("activate_sandbox_mod_preset", { presetId });
    console.log("[SandboxPreset] activation result:", result);
    modProfileManagerState.activations.set(presetId, result);
    modProfileManagerState.progressLog = Array.isArray(result.progress_log) ? result.progress_log : [];
    await renderSandboxProgressList(modProfileManagerState.progressLog);
    await renderSandboxResult(result);
    await renderSandboxPresetCards();

    if (result.success) {
      await setModProfileStatus("modals.mod_profile_manager.sandbox.status.activated", "success");
      await showSandboxPresetPopup(
        "success",
        await window.t("modals.mod_profile_manager.sandbox.popup.success_title"),
        await window.t("modals.mod_profile_manager.sandbox.popup.success_message", { title: result.title || "" })
      );
      return;
    }

    const title = await sandboxErrorTitle(result.error_code);
    const message = await sandboxErrorMessage(result);
    const stateKey = result.error_code === "mod_not_found"
      ? "modals.mod_profile_manager.sandbox.status.mod_missing"
      : "modals.mod_profile_manager.sandbox.status.error";
    await setModProfileStatus(stateKey, result.error_code === "mod_not_found" ? "warning" : "error");
    await showSandboxPresetPopup("error", title, message);
  } catch (error) {
    console.error("Sandbox preset activation failed:", error);
    modProfileManagerState.progressLog = [];
    await renderSandboxProgressList([]);
    await setModProfileStatus("modals.mod_profile_manager.sandbox.status.error", "error");
    await showSandboxPresetPopup(
      "error",
      await window.t("modals.mod_profile_manager.sandbox.popup.error_title"),
      await window.t("modals.mod_profile_manager.sandbox.popup.save_failed_message")
    );
  } finally {
    modProfileManagerState.activatingPresetId = null;
    await renderSandboxPresetCards();
  }
}

modSandboxReloadBtn?.addEventListener("click", loadSandboxPresetsIntoModal);
modSteamConsoleBtn?.addEventListener("click", async () => {
  try {
    await window.invoke("open_steam_console");
  } catch (error) {
    console.error("Open Steam console failed:", error);
    window.showToast?.("toasts.mod_sandbox_open_steam_console_failed", "error");
  }
});
modalModProfileManagerClose?.addEventListener("click", closeModProfileManagerModal);
modalModProfileManager?.addEventListener("click", (event) => {
  if (event.target === modalModProfileManager) {
    closeModProfileManagerModal();
  }
});

window.openModProfileManagerModal = openModProfileManagerModal;

if (saveImportSavesBtn) {
  saveImportSavesBtn.addEventListener("click", () => {
    openProfileSharingPage("import");
  });
}

if (saveExportSavesBtn) {
  saveExportSavesBtn.addEventListener("click", () => {
    openProfileSharingPage("export");
  });
}
