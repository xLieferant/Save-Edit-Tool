import { tools } from "./tools.js";

/* --------------------------------------------------------------
   TOOL LOADER UND TAB HANDLING
-------------------------------------------------------------- */
const container = document.querySelector("#tool-container");
const navButtons = document.querySelectorAll(".editor-tabs .nav-btn");
export let activeTab = "profile";
let loadToolsRenderId = 0;

export async function loadTools(tab) {
  console.log(`[app.js] Lade Tools für Tab: ${tab}`);

  activeTab = tab;
  if (!container) return;
  container.innerHTML = "";
  const renderId = ++loadToolsRenderId;
  const toolList = tools[tab] || [];
  const tabLabelKeyMap = {
    truck: "editor.tab.truck",
    trailer: "editor.tab.trailer",
    profile: "editor.tab.profile",
    settings: "editor.tab.settings",
  };

  document.dispatchEvent(new CustomEvent("editor-tab-changed", { detail: { tab } }));

  if (!toolList.length) {
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

navButtons.forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelector(".editor-tabs .nav-btn.active")?.classList.remove("active");
    btn.classList.add("active");
    loadTools(btn.dataset.tab);
  });
});

// Default Tab
const defaultTabBtn = document.querySelector(".editor-tabs .nav-btn.active");
if (defaultTabBtn) {
  if (typeof window.t === "function") {
    loadTools(defaultTabBtn.dataset.tab);
  } else {
    window.addEventListener(
      "translations-ready",
      () => loadTools(defaultTabBtn.dataset.tab),
      { once: true }
    );
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const savedTheme = localStorage.getItem("theme") || "neon";
  document.body.classList.remove("theme-dark", "theme-light", "theme-neon");
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
      const input = document.createElement("input");
      input.type = "checkbox";
      input.id = item.id;
      input.checked = Boolean(item.value ?? 0);
      input.className = "modal-checkbox";
      control.appendChild(input);
      inputs.push(input);
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
