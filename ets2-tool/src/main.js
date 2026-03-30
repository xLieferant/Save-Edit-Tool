import { loadTools, activeTab, openCloneProfileModal, openModalMulti, openModalText } from "./app.js";
import { updateToolImagesForGame } from "./tools.js";
import { applySetting } from "./js/applySetting.js";
import { checkUpdaterOnStartup, manualUpdateCheck } from "./js/updater.js";

const { app } = window.__TAURI__;
const { openUrl } = window.__TAURI__.opener;
const { invoke, convertFileSrc } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

let lastSelectedGame = null;
window.invoke = invoke;
window.applySetting = applySetting;

function formatTelemetryNumber(value, digits = 0) {
  return Number(value ?? 0).toLocaleString(undefined, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
}

function getThemeFallbackIcon() {
  return document.body.classList.contains("theme-light")
    ? "images/icon_Black.png"
    : "images/icon_White.png";
}

function resolveProfileIcon(profile) {
  if (profile?.avatar) {
    return profile.avatar.startsWith("data:")
      ? profile.avatar
      : convertFileSrc(profile.avatar);
  }
  return getThemeFallbackIcon();
}

function handleIconError(img) {
  img.onerror = null;
  img.src = getThemeFallbackIcon();
  img.removeAttribute("data-has-avatar");
}

function updateAllProfileIcons() {
  const fallback = getThemeFallbackIcon();
  document.querySelectorAll(".profile-icon-dropdown").forEach((img) => {
    if (!img.dataset.hasAvatar) {
      img.src = fallback;
    }
  });
  if (!window.selectedProfileHasAvatar) {
    document
      .querySelectorAll("#activeProfileIcon, .nav-icon-profile")
      .forEach((img) => (img.src = fallback));
  }
}

new MutationObserver(updateAllProfileIcons).observe(document.body, {
  attributes: true,
  attributeFilter: ["class"],
});

async function t(key, params = {}) {
  try {
    let text = await invoke("translate_command", { key });
    for (const [k, v] of Object.entries(params)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }
    return text;
  } catch (error) {
    console.error("Translation error:", error);
    return key;
  }
}

async function translateUI() {
  const elements = document.querySelectorAll("[data-translate]");
  for (const el of elements) {
    const key = el.getAttribute("data-translate");
    el.textContent = await t(key);
  }
}

window.t = t;
window.translateUI = translateUI;

window.showToast = async function (messageOrKey, options = {}, type = "info") {
  const resolvedOptions = typeof options === "string" ? {} : options;
  const resolvedType = typeof options === "string" ? options : type;
  const message = await t(messageOrKey, resolvedOptions);
  const toast = document.createElement("div");
  toast.className = `toast toast-${resolvedType}`;
  toast.innerHTML = `<span class="toast-icon">${resolvedType === "success" ? "✓" : resolvedType === "error" ? "✕" : resolvedType === "warning" ? "⚠" : "ℹ"}</span><span class="toast-text"></span>`;
  toast.querySelector(".toast-text").textContent = message;
  document.body.appendChild(toast);
  requestAnimationFrame(() => toast.classList.add("show"));
  setTimeout(() => {
    toast.classList.remove("show");
    setTimeout(() => toast.remove(), 300);
  }, 4500);
};

async function appVersion() {
  try {
    return await app.getVersion();
  } catch (error) {
    console.error("Version load failed:", error);
    return "0.0.0";
  }
}

async function logUserAction(action, stage = "start") {
  try {
    await invoke("log_user_action", { action, stage });
  } catch (error) {
    console.warn("User log failed:", error);
  }
}

window.logUserAction = logUserAction;

document.addEventListener("DOMContentLoaded", async () => {
  await translateUI();
  document.body.classList.add("mode-editor");

  const refs = {
    profileStatus: document.getElementById("profile-status"),
    profileNameDisplay: document.getElementById("profileNameDisplay"),
    profileDropdownList: document.getElementById("profileDropdownList"),
    saveNameDisplay: document.getElementById("saveName"),
    saveDropdownList: document.getElementById("saveDropdownList"),
    openSaveModalBtn: document.getElementById("openSaveModal"),
    ets2Btn: document.getElementById("ets2Btn"),
    atsBtn: document.getElementById("atsBtn"),
    editorModeBtn: document.getElementById("editorModeBtn"),
    careerModeBtn: document.getElementById("careerModeBtn"),
    statusGameRunning: document.getElementById("statusGameRunning"),
    statusPluginInstalled: document.getElementById("statusPluginInstalled"),
    statusSdkConnected: document.getElementById("statusSdkConnected"),
    careerHeroTitle: document.getElementById("careerHeroTitle"),
    careerGameLabel: document.getElementById("careerGameLabel"),
    careerConnectionNote: document.getElementById("careerConnectionNote"),
    careerSpeedDial: document.getElementById("careerSpeedDial"),
    careerSpeedValue: document.getElementById("careerSpeedValue"),
    careerGearValue: document.getElementById("careerGearValue"),
    careerFuelValue: document.getElementById("careerFuelValue"),
    careerFuelPercent: document.getElementById("careerFuelPercent"),
    careerFuelBarFill: document.getElementById("careerFuelBarFill"),
    careerRpmValue: document.getElementById("careerRpmValue"),
    careerOdometerValue: document.getElementById("careerOdometerValue"),
    careerMapScaleValue: document.getElementById("careerMapScaleValue"),
    versionBtn: document.getElementById("versionBtn"),
    websiteBtn: document.getElementById("websiteBtn"),
    youtubeBtn: document.getElementById("youtubeBtn"),
    patreonBtn: document.getElementById("patreonBtn"),
    githubBtn: document.getElementById("githubBtn"),
    refreshBtn: document.getElementById("refreshBtn"),
    cloneBtn: document.getElementById("cloneProfileBtn"),
  };

  const careerText = {
    waiting: await t("career.status.awaiting_signal"),
    live: await t("career.status.sdk_live"),
    paused: await t("career.status.sdk_paused"),
    missingPlugin: await t("career.status.plugin_missing"),
    gameStopped: await t("career.status.game_stopped"),
  };

  const setLamp = (element, active) => element?.classList.toggle("is-active", Boolean(active));
  const setCareerGame = (game) => {
    const label = (game || "ets2").toUpperCase();
    if (refs.careerHeroTitle) refs.careerHeroTitle.textContent = label;
    if (refs.careerGameLabel) refs.careerGameLabel.textContent = label;
  };

  const applyHubMode = (mode) => {
    const isCareer = mode === "career";
    document.body.classList.toggle("mode-career", isCareer);
    document.body.classList.toggle("mode-editor", !isCareer);
    refs.editorModeBtn?.classList.toggle("active", !isCareer);
    refs.careerModeBtn?.classList.toggle("active", isCareer);
  };

  const renderTelemetry = (data) => {
    const speed = Number(data?.speed_kph ?? 0);
    const fuel = Number(data?.fuel_liters ?? 0);
    const capacity = Number(data?.fuel_capacity_liters ?? 0);
    const ratio = capacity > 0 ? Math.max(0, Math.min(fuel / capacity, 1)) : 0;
    if (refs.careerSpeedDial) refs.careerSpeedDial.style.setProperty("--dial-progress", String(Math.min(speed / 180, 1)));
    if (refs.careerSpeedValue) refs.careerSpeedValue.textContent = String(Math.round(speed));
    if (refs.careerGearValue) {
      const gear = Number(data?.gear ?? 0);
      refs.careerGearValue.textContent = gear === 0 ? "N" : gear > 0 ? String(gear) : `R${Math.abs(gear)}`;
    }
    if (refs.careerFuelValue) {
      refs.careerFuelValue.textContent =
        capacity > 0
          ? `${formatTelemetryNumber(fuel, 1)} / ${formatTelemetryNumber(capacity, 1)} L`
          : `${formatTelemetryNumber(fuel, 1)} L`;
    }
    if (refs.careerFuelPercent) refs.careerFuelPercent.textContent = `${Math.round(ratio * 100)}%`;
    if (refs.careerFuelBarFill) refs.careerFuelBarFill.style.setProperty("--fuel-progress", String(ratio));
    if (refs.careerRpmValue) refs.careerRpmValue.textContent = formatTelemetryNumber(data?.engine_rpm ?? 0, 0);
    if (refs.careerOdometerValue) refs.careerOdometerValue.textContent = `${formatTelemetryNumber(data?.odometer_km ?? 0, 1)} km`;
    if (refs.careerMapScaleValue) refs.careerMapScaleValue.textContent = formatTelemetryNumber(data?.map_scale ?? 0, 2);
    if (refs.careerConnectionNote) refs.careerConnectionNote.textContent = Number(data?.paused ?? 0) === 1 ? careerText.paused : careerText.live;
  };

  const renderCareerStatus = (status) => {
    const gameRunning = Boolean(status?.ets2_running || status?.ats_running);
    setLamp(refs.statusGameRunning, gameRunning);
    setLamp(refs.statusPluginInstalled, status?.plugin_installed);
    setLamp(refs.statusSdkConnected, status?.bridge_connected);
    setCareerGame(status?.active_game || lastSelectedGame || "ets2");
    if (!refs.careerConnectionNote) return;
    if (status?.bridge_connected) refs.careerConnectionNote.textContent = careerText.live;
    else if (!status?.plugin_installed) refs.careerConnectionNote.textContent = careerText.missingPlugin;
    else if (!gameRunning) refs.careerConnectionNote.textContent = careerText.gameStopped;
    else refs.careerConnectionNote.textContent = careerText.waiting;
  };

  refs.editorModeBtn?.addEventListener("click", () => invoke("hub_set_mode", { mode: "utility" }).catch(console.error));
  refs.careerModeBtn?.addEventListener("click", () => invoke("hub_set_mode", { mode: "career" }).catch(console.error));

  listen("hub://mode_changed", (event) => applyHubMode(event.payload.mode)).catch(console.error);
  listen("career://status", (event) => renderCareerStatus(event.payload)).catch(console.error);
  listen("career://telemetry_tick", (event) => renderTelemetry(event.payload)).catch(console.error);

  try {
    applyHubMode(await invoke("hub_get_mode"));
  } catch {
    applyHubMode("utility");
  }

  try {
    const selectedGame = await invoke("get_selected_game");
    setCareerGame(selectedGame);
    lastSelectedGame = selectedGame;
  } catch {}

  try {
    renderCareerStatus(await invoke("career_get_status"));
  } catch {}

  try {
    setLamp(refs.statusPluginInstalled, await invoke("get_plugin_status"));
  } catch {}

  if (refs.versionBtn) {
    refs.versionBtn.textContent = `v${await appVersion()}`;
    refs.versionBtn.addEventListener("click", () => manualUpdateCheck(showToast));
  }

  setTimeout(() => checkUpdaterOnStartup(showToast), 2000);

  refs.websiteBtn?.addEventListener("click", () => openUrl("https://www.xlieferant.dev/"));
  refs.youtubeBtn?.addEventListener("click", () => openUrl("https://www.youtube.com/@xLieferant"));
  refs.patreonBtn?.addEventListener("click", () => openUrl("https://www.patreon.com/cw/xLieferant"));
  refs.githubBtn?.addEventListener("click", () => openUrl("https://github.com/xLieferant/Save-Edit-Tool"));

  const switchGame = async (game) => {
    try {
      await invoke("set_selected_game", { game });
      localStorage.setItem("ets2_force_profile_picker_open", "1");
      location.reload();
    } catch (error) {
      showToast("toasts.generic_error_prefix", { error: error.toString() }, "error");
    }
  };

  refs.ets2Btn?.addEventListener("click", () => switchGame("ets2"));
  refs.atsBtn?.addEventListener("click", () => switchGame("ats"));

  window.selectedProfilePath = null;
  window.selectedSavePath = null;
  window.currentSavePath = null;
  window.currentProfileData = {};
  window.currentQuicksaveData = {};
  window.readSaveGameConfig = {};
  window.baseConfig = {};
  window.allTrucks = [];
  window.playerTruck = null;
  window.allTrailers = [];
  window.playerTrailer = null;
  window.extractPlateText = (plate) => (plate ? plate.replace(/^"|"$/g, "") : "");

  const closeDropdowns = () => {
    refs.profileDropdownList.classList.remove("show");
    refs.saveDropdownList.classList.remove("show");
  };

  document.addEventListener("click", (event) => {
    if (!event.target.closest(".profile-picker")) refs.profileDropdownList.classList.remove("show");
    if (!event.target.closest(".save-picker")) refs.saveDropdownList.classList.remove("show");
  });

  document.querySelector(".profile-picker")?.addEventListener("click", (event) => {
    if (event.target.closest(".custom-dropdown-list")) return;
    event.stopPropagation();
    const open = refs.profileDropdownList.classList.contains("show");
    closeDropdowns();
    if (!open) refs.profileDropdownList.classList.add("show");
  });

  document.querySelector(".save-picker")?.addEventListener("click", (event) => {
    if (event.target.closest(".dropdown-list") || !window.selectedProfilePath) return;
    event.stopPropagation();
    const open = refs.saveDropdownList.classList.contains("show");
    closeDropdowns();
    if (!open) refs.saveDropdownList.classList.add("show");
  });

  if (localStorage.getItem("ets2_force_profile_picker_open") === "1") {
    localStorage.removeItem("ets2_force_profile_picker_open");
    refs.profileDropdownList.classList.add("show");
  }

  const loadProfileData = async () => {
    window.currentProfileData = await invoke("read_all_save_data");
  };
  const loadQuicksave = async () => {
    window.currentQuicksaveData = await invoke("quicksave_game_info");
  };
  const loadProfileSaveConfig = async () => {
    window.readSaveGameConfig = await invoke("read_save_config", { profilePath: window.selectedProfilePath });
  };
  const loadBaseConfig = async () => {
    window.baseConfig = await invoke("read_base_config");
  };
  const loadAllTrucks = async () => {
    try {
      window.playerTruck = await invoke("get_player_truck", { profilePath: window.selectedProfilePath });
      window.allTrucks = [window.playerTruck];
    } catch {
      window.playerTruck = null;
      window.allTrucks = [];
    }
  };
  const loadAllTrailers = async () => {
    try {
      const trailer = await invoke("get_player_trailer", { profilePath: window.selectedProfilePath });
      window.playerTrailer = trailer || null;
      window.allTrailers = trailer ? [trailer] : [];
    } catch {
      window.playerTrailer = null;
      window.allTrailers = [];
    }
  };

  window.loadProfileData = loadProfileData;
  window.loadQuicksave = loadQuicksave;
  window.loadProfileSaveConfig = loadProfileSaveConfig;
  window.loadBaseConfig = loadBaseConfig;
  window.loadAllTrucks = loadAllTrucks;
  window.loadAllTrailers = loadAllTrailers;

  const syncSelectedGameUi = async () => {
    try {
      const game = await invoke("get_selected_game");
      const previousGame = lastSelectedGame;
      lastSelectedGame = game;
      setCareerGame(game);
      setLamp(refs.statusPluginInstalled, await invoke("get_plugin_status"));
      refs.ets2Btn.classList.toggle("active", game !== "ats");
      refs.ets2Btn.disabled = game !== "ats";
      refs.atsBtn.classList.toggle("active", game === "ats");
      refs.atsBtn.disabled = game === "ats";
      if (game !== previousGame) {
        updateToolImagesForGame(game);
        loadTools(activeTab);
      } else {
        updateToolImagesForGame(game);
      }
    } catch (error) {
      console.warn("Game sync failed:", error);
    }
  };

  const loadSelectedSave = async () => {
    window.logUserAction("load_save", "start");
    try {
      refs.profileStatus.textContent = "Loading save...";
      await loadProfileData();
      await loadQuicksave();
      await loadProfileSaveConfig();
      await loadBaseConfig();
      await loadAllTrucks();
      await loadAllTrailers();
      updateUIWithCurrentQuicksave();
      refs.profileStatus.textContent = "Save loaded successfully";
      showToast("toasts.save_loaded_success", {}, "success");
      loadTools(activeTab);
      window.logUserAction("load_save", "success");
    } catch (error) {
      console.error(error);
      showToast("toasts.save_load_error", {}, "error");
      window.logUserAction("load_save", "error");
    }
  };

  const scanSavesForProfile = async () => {
    if (!window.selectedProfilePath) return;
    refs.saveDropdownList.innerHTML = "";
    refs.openSaveModalBtn.disabled = false;
    try {
      const saves = (await invoke("find_profile_saves", { profilePath: window.selectedProfilePath }))
        .filter((save) => save.success && save.kind !== "Invalid")
        .sort((a, b) => {
          const priority = (folder) => folder === "quicksave" ? 0 : folder === "autosave" ? 1 : 2;
          const pA = priority(a.folder.toLowerCase());
          const pB = priority(b.folder.toLowerCase());
          return pA !== pB ? pA - pB : b.folder.localeCompare(a.folder, undefined, { numeric: true });
        });
      saves.forEach((save) => {
        const item = document.createElement("div");
        item.className = "dropdown-item";
        item.textContent =
          save.folder.toLowerCase() === "quicksave"
            ? "~ Quicksave ~"
            : save.folder.toLowerCase() === "autosave"
              ? "~ Autosave ~"
              : `${save.name ?? save.folder} [${save.folder}]`;
        item.addEventListener("click", async () => {
          window.selectedSavePath = save.path;
          window.currentSavePath = save.path;
          refs.saveNameDisplay.textContent = save.name ?? save.folder;
          refs.saveDropdownList.classList.remove("show");
          await invoke("load_profile", { profilePath: window.selectedProfilePath, savePath: save.path });
          await loadSelectedSave();
        });
        refs.saveDropdownList.appendChild(item);
      });
    } catch (error) {
      console.error(error);
      showToast("toasts.scan_saves_error", {}, "error");
    }
  };

  const loadSelectedProfile = async () => {
    if (!window.selectedProfilePath) return;
    window.logUserAction("load_profile", "start");
    try {
      refs.profileStatus.textContent = "Loading profile...";
      refs.saveDropdownList.innerHTML = "";
      window.selectedSavePath = null;
      window.currentSavePath = null;
      refs.saveNameDisplay.textContent = "Select a save";
      await invoke("set_active_profile", { profilePath: window.selectedProfilePath });
      await scanSavesForProfile();
      try {
        await loadBaseConfig();
        await loadProfileSaveConfig();
      } catch {}
      window.currentProfileData = {};
      window.currentQuicksaveData = {};
      window.allTrucks = [];
      window.playerTruck = null;
      window.allTrailers = [];
      window.playerTrailer = null;
      const cached = JSON.parse(localStorage.getItem("ets2_profiles_cache") || "[]");
      const profileInfo = cached.find((profile) => profile.path === window.selectedProfilePath);
      window.selectedProfileHasAvatar = !!profileInfo?.avatar;
      const iconSrc = resolveProfileIcon(profileInfo);
      const footerIcon = document.getElementById("activeProfileIcon");
      if (footerIcon) {
        footerIcon.src = iconSrc;
        if (window.selectedProfileHasAvatar) footerIcon.onerror = () => handleIconError(footerIcon);
      }
      refs.profileStatus.textContent = "Profile loaded. Please select a save.";
      showToast("toasts.profile_loaded_select_save", {}, "info");
      loadTools(activeTab);
      window.logUserAction("load_profile", "success");
    } catch (error) {
      console.error(error);
      refs.profileStatus.textContent = "Error loading profile";
      showToast("toasts.profile_load_error", {}, "error");
      window.logUserAction("load_profile", "error");
    }
  };

  const createProfileItem = (profile) => {
    const item = document.createElement("div");
    item.className = "dropdown-item";
    const img = document.createElement("img");
    img.src = resolveProfileIcon(profile);
    img.className = "profile-icon-dropdown";
    if (profile.avatar) {
      img.dataset.hasAvatar = "true";
      img.onerror = () => handleIconError(img);
    }
    const label = document.createElement("span");
    label.textContent = profile.name;
    item.appendChild(img);
    item.appendChild(label);
    item.addEventListener("click", async () => {
      window.selectedProfilePath = profile.path;
      refs.profileNameDisplay.textContent = profile.name;
      refs.profileDropdownList.classList.remove("show");
      localStorage.setItem("ets2_last_profile", profile.path);
      try {
        await invoke("save_last_profile", { profilePath: profile.path });
      } catch {}
      await loadSelectedProfile();
    });
    return item;
  };

  const scanProfiles = async ({ saveToBackend = true, showToasts = true } = {}) => {
    refs.profileStatus.textContent = "Scanning profiles...";
    refs.profileDropdownList.innerHTML = "";
    window.logUserAction("scan_profiles", "start");
    try {
      await syncSelectedGameUi();
      const profiles = await invoke("find_ets2_profiles");
      refs.profileStatus.textContent = `${profiles.length} profiles found`;
      if (showToasts) showToast("toasts.profiles_found", {}, "success");
      profiles.filter((profile) => profile.success).forEach((profile) => {
        refs.profileDropdownList.appendChild(createProfileItem(profile));
      });
      localStorage.setItem("ets2_profiles_cache", JSON.stringify(profiles));
      if (saveToBackend) {
        await invoke("save_profiles_cache", {
          profiles: profiles.map((profile) => ({
            path: profile.path,
            name: profile.name ?? null,
            avatar: profile.avatar ?? null,
            success: !!profile.success,
            message: profile.message ?? null,
          })),
        });
      }
      const remoteLast = await invoke("read_last_profile").catch(() => null);
      const last = remoteLast || localStorage.getItem("ets2_last_profile");
      if (last) {
        const matched = profiles.find((profile) => profile.path === last && profile.success);
        if (matched) {
          window.selectedProfilePath = matched.path;
          refs.profileNameDisplay.textContent = matched.name ?? "Unknown";
          await loadSelectedProfile();
          return;
        }
      }
      window.logUserAction("scan_profiles", "success");
    } catch (error) {
      console.error(error);
      refs.profileStatus.textContent = "Scan failed";
      showToast("toasts.no_profiles_found", {}, "error");
      window.logUserAction("scan_profiles", "error");
    }
  };

  refs.refreshBtn?.addEventListener("click", () => scanProfiles({ saveToBackend: true, showToasts: true }));

  refs.cloneBtn?.addEventListener("click", async () => {
    if (!window.selectedProfilePath) {
      showToast("toasts.no_profile_selected", {}, "warning");
      return;
    }
    const choice = await openModalMulti("Manage Profile", [{
      type: "dropdown",
      id: "action",
      label: "Action",
      value: "Duplicate",
      options: ["Duplicate", "Rename"],
    }]);
    if (!choice) return;
    if (choice.action === "Duplicate") return openCloneProfileModal();
    const currentName = refs.profileNameDisplay.textContent;
    const newName = await openModalText("Rename Profile", "New Name", currentName);
    if (newName && newName.trim() !== "" && newName !== currentName) {
      try {
        window.selectedProfilePath = await invoke("profile_rename", { newName: newName.trim() });
        refs.profileNameDisplay.textContent = newName.trim();
        showToast("toasts.profile_renamed_success", {}, "success");
        await scanProfiles({ saveToBackend: true, showToasts: false });
        await loadSelectedProfile();
      } catch (error) {
        showToast("toasts.profile_rename_error", { error: error.toString() }, "error");
      }
    }
  });

  window.handleCopyControls = async function () {
    if (!window.selectedProfilePath) {
      showToast("toasts.no_source_profile_selected", {}, "warning");
      return;
    }
    const profiles = await invoke("find_ets2_profiles");
    const targets = profiles.filter((profile) => profile.success && profile.path !== window.selectedProfilePath);
    if (!targets.length) {
      showToast("toasts.no_other_profiles", {}, "warning");
      return;
    }
    const result = await openModalMulti("Copy Controls", [{
      type: "dropdown",
      id: "target",
      label: "Target Profile",
      value: `${targets[0].name} [${targets[0].path}]`,
      options: targets.map((profile) => `${profile.name} [${profile.path}]`),
    }]);
    if (!result?.target) return;
    const target = targets.find((profile) => `${profile.name} [${profile.path}]` === result.target);
    if (!target) return;
    await invoke("copy_profile_controls", {
      sourceProfilePath: window.selectedProfilePath,
      targetProfilePath: target.path,
    });
    showToast("toasts.copy_controls_success", {}, "success");
  };

  window.handleMoveMods = async function () {
    if (!window.selectedProfilePath) {
      showToast("toasts.no_source_profile_selected", {}, "warning");
      return;
    }
    const profiles = await invoke("find_ets2_profiles");
    const targets = profiles.filter((profile) => profile.success && profile.path !== window.selectedProfilePath);
    if (!targets.length) {
      showToast("toasts.no_other_valid_profiles", {}, "warning");
      return;
    }
    const result = await openModalMulti("Move Mods", [{
      type: "dropdown",
      id: "target",
      label: "Target Profile",
      value: `${targets[0].name} [${targets[0].path}]`,
      options: targets.map((profile) => `${profile.name} [${profile.path}]`),
    }]);
    if (!result?.target) return;
    const target = targets.find((profile) => `${profile.name} [${profile.path}]` === result.target);
    if (!target) return;
    const message = await invoke("copy_mods_to_profile", { targetProfilePath: target.path });
    const count = message.match(/\d+/)?.[0] ?? "?";
    showToast("toasts.move_mods_success", { count }, "success");
  };

  window.showLanguagePicker = async function () {
    try {
      const languages = await invoke("get_available_languages_command");
      const currentLanguage = await invoke("get_current_language_command");

      if (!languages?.length) {
        showToast("toasts.load_languages_error", { error: "No languages available" }, "error");
        return;
      }

      const optionLabels = languages.reduce((acc, language) => {
        acc[language.code] = language.name;
        return acc;
      }, {});

      const result = await openModalMulti("tools.settings.language.modalTextTitle", [
        {
          type: "dropdown",
          id: "language",
          label: "label.label_language",
          value: currentLanguage,
          options: languages.map((language) => language.code),
          optionLabels,
        },
      ]);

      if (!result?.language || result.language === currentLanguage) {
        return;
      }

      const message = await invoke("set_language_command", { language: result.language });
      showToast(message, {}, "success");
      location.reload();
    } catch (error) {
      console.error("Language picker failed:", error);
      showToast("toasts.load_languages_error", { error: error.toString() }, "error");
    }
  };

  setInterval(async () => {
    if (!window.selectedSavePath) return;
    try {
      await loadQuicksave();
      updateUIWithCurrentQuicksave();
    } catch {}
  }, 300000);

  try {
    const cached = await invoke("read_profiles_cache");
    if (cached?.length) {
      refs.profileDropdownList.innerHTML = "";
      cached.filter((profile) => profile.success).forEach((profile) => {
        refs.profileDropdownList.appendChild(createProfileItem(profile));
      });
    }
  } catch {}

  await scanProfiles({ saveToBackend: true, showToasts: true });
  window.dispatchEvent(new Event("translations-ready"));
});
