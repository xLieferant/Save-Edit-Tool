import { loadTools, activeTab, openCloneProfileModal, openModalMulti, openModalText } from "./app.js";
import { applySetting } from "./js/applySetting.js";
import { checkUpdaterOnStartup, manualUpdateCheck } from "./js/updater.js";

const { app } = window.__TAURI__;
const { openUrl } = window.__TAURI__.opener;
const { invoke, convertFileSrc } = window.__TAURI__.core;
window.invoke = invoke; // global verfügbar

// -----------------------------
// ICON & THEME LOGIC
// -----------------------------
function getThemeFallbackIcon() {
  const isLight = document.body.classList.contains("theme-light");
  return isLight ? "images/icon_Black.png" : "images/icon_White.png";
}

function resolveProfileIcon(profile) {
  if (profile && profile.avatar) {
    if (profile.avatar.startsWith("data:")) {
      return profile.avatar;
    }
    return convertFileSrc(profile.avatar);
  }
  return getThemeFallbackIcon();
}

function handleIconError(img) {
  img.onerror = null; // Prevent infinite loop
  img.src = getThemeFallbackIcon();
  img.removeAttribute("data-has-avatar");
}

function updateAllProfileIcons() {
  const fallbackIcon = getThemeFallbackIcon();
  
  // Update Dropdown Items that use fallback
  document.querySelectorAll(".profile-icon-dropdown").forEach(img => {
    if (!img.dataset.hasAvatar) {
      img.src = fallbackIcon;
    }
  });

  // Update Active Profile Icons (Footer & Sidebar)
  const activeHasAvatar = window.selectedProfileHasAvatar;
  if (!activeHasAvatar) {
     const activeIcons = document.querySelectorAll("#activeProfileIcon, .nav-icon-profile");
     activeIcons.forEach(img => img.src = fallbackIcon);
  }
}

// Watch for theme changes
const themeObserver = new MutationObserver(() => {
  updateAllProfileIcons();
});
themeObserver.observe(document.body, { attributes: true, attributeFilter: ["class"] });


// -----------------------------
// TOAST-FUNKTION
// -----------------------------
window.showToast = async function (messageOrKey, options = {}, type = "info") {
  const message = await t(messageOrKey, options); // Translate the key

  const toast = document.createElement("div");
  toast.className = `toast toast-${type}`;

  const icon = document.createElement("span");
  icon.className = "toast-icon";
  icon.innerHTML = getToastIcon(type);

  const text = document.createElement("span");
  text.className = "toast-text";
  text.textContent = message;

  toast.appendChild(icon);
  toast.appendChild(text);
  document.body.appendChild(toast);

  requestAnimationFrame(() => toast.classList.add("show"));

  setTimeout(() => {
    toast.classList.remove("show");
    setTimeout(() => toast.remove(), 300);
  }, 4500);
};

function getToastIcon(type) {
  switch (type) {
    case "success":
      return "✔";
    case "error":
      return "✖";
    case "warning":
      return "⚠";
    case "info":
    default:
      return "ℹ";
  }
}

// -----------------------------
// APP VERSION
// -----------------------------
async function appVersion() {
  try {
    return await app.getVersion();
  } catch (err) {
    console.error("Version konnte nicht geladen werden:", err);
    return "0.0.0";
  }
}

async function initVersionInfo() {
  const version = await appVersion();
  const versionElement = document.querySelector(".version-info");
  if (versionElement) {
    versionElement.textContent = `v${version}`;
  }
}

// -----------------------------
// UPDATE UI WITH DATA
// -----------------------------
function updateUIWithCurrentQuicksave() {
  // Update XP Display
  const xpDisplay = document.querySelector("#xpShow");
  if (xpDisplay && window.currentProfileData?.xp !== undefined) {
    xpDisplay.textContent = `XP: ${window.currentProfileData.xp.toLocaleString()}`;
  }

  // Update Money Display
  const moneyDisplay = document.querySelector("#moneyShow");
  if (moneyDisplay && window.currentProfileData?.money !== undefined) {
    moneyDisplay.textContent = `Geld: ${window.currentProfileData.money.toLocaleString()} €`;
  }

  // Update Skills Display (if needed)
  // Add more UI updates as needed
}

// -----------------------------
// DOM READY – ZENTRALE INIT
// -----------------------------
document.addEventListener("DOMContentLoaded", () => {
  console.log("[main.js] DOM vollständig geladen.");

  // Initial UI translation
  translateUI();

  // Periodic data refresh (every 5 minutes)
  setInterval(async () => {
    if (window.selectedSavePath) {
      try {
        window.currentQuicksaveData = await invoke("quicksave_game_info");
        updateUIWithCurrentQuicksave();
      } catch (e) {
        console.warn("Periodic refresh failed:", e);
      }
    }
  }, 300000); // 5 minutes

  // -----------------------------
  // BASIS INIT
  // -----------------------------
  initVersionInfo();

  // Auto-Updater (verzögert, stabil)
  setTimeout(() => {
    checkUpdaterOnStartup(showToast);
  }, 2000);

  // Load and display current language
  (async function initLanguage() {
    try {
      const lang = await invoke('get_current_language_command');
      const message = await invoke('translate_command', { key: 'toasts.language_loaded' });
      console.log(message); // Will show "Language loaded: English" or "Sprache geladen: Deutsch"
    } catch (e) {
      console.warn('Could not load language:', e);
    }
  })();

  // -----------------------------
  // UPDATE BUTTONS
  // -----------------------------
  const versionBtn = document.getElementById("versionBtn");
  if (versionBtn) {
    versionBtn.addEventListener("click", () => {
      manualUpdateCheck(showToast);
    });
  }

  const checkUpdateBtn = document.getElementById("checkUpdateBtn");
  if (checkUpdateBtn) {
    checkUpdateBtn.addEventListener("click", () => {
      manualUpdateCheck(showToast);
    });
  }

  // -----------------------------
  // DOM ELEMENTE
  // -----------------------------
  const scanBtn = document.querySelector("#refreshBtn");
  const profileNameDisplay = document.querySelector("#profileNameDisplay");
  const profileDropdownList = document.querySelector("#profileDropdownList");
  const openProfileModalBtn = document.querySelector("#openProfileModal");
  const profileStatus = document.querySelector("#profile-status");

  // SAVE PICKER
  const saveNameDisplay = document.querySelector("#saveName");
  const saveDropdownList = document.querySelector("#saveDropdownList");
  const openSaveModalBtn = document.querySelector("#openSaveModal");

  const moneyBtn = document.querySelector("#save-money-btn");
  const levelBtn = document.querySelector("#save-level-btn");
  const editStatus = document.querySelector("#edit-status");

  const youtubeBtn = document.querySelector("#youtubeBtn");
  const patreonBtn = document.querySelector("#patreonBtn");
  const githubBtn = document.querySelector("#githubBtn");

  // GAME SWITCHER
  const ets2Btn = document.getElementById("ets2Btn");
  const atsBtn = document.getElementById("atsBtn");

  async function switchGame(game) {
    try {
      await invoke("set_selected_game", { game });
      
      // Full reload avoids duplicated tabs/modals after game switch
      localStorage.setItem("ets2_force_profile_picker_open", "1");
      location.reload();
      
    } catch (e) {
      console.error("Failed to switch game:", e);
      showToast("toasts.generic_error_prefix", { error: e.toString() }, "error");
    }
  }

  if (ets2Btn) {
    ets2Btn.addEventListener("click", () => switchGame("ets2"));
  }
  if (atsBtn) {
    atsBtn.addEventListener("click", () => switchGame("ats"));
  }

  // Global state for selected paths
  window.selectedProfilePath = null;
  window.selectedSavePath = null;
  window.currentSavePath = null;

  // -----------------------------
  // GLOBAL STATE
  // -----------------------------
  window.currentProfileData = {};
  window.currentQuicksaveData = {};
  window.readSaveGameConfig = {};
  window.baseConfig = {};
  window.allTrucks = [];
  window.playerTruck = null;
  window.allTrailers = [];
  window.playerTrailer = null;
  window.translateUI = translateUI;
  window.applySetting = applySetting;

  // Helper to extract plate text
  window.extractPlateText = function(plate) {
    if (!plate) return "";
    // Remove quotes if present
    return plate.replace(/^"|"$/g, '');
  };

  // -----------------------------
  // DROPDOWN
  // -----------------------------
  function closeAllDropdowns() {
    profileDropdownList.classList.remove("show");
    saveDropdownList.classList.remove("show");
  }

  const forceProfilePickerOpen = localStorage.getItem("ets2_force_profile_picker_open") === "1";
  if (forceProfilePickerOpen) {
    localStorage.removeItem("ets2_force_profile_picker_open");
    closeAllDropdowns();
    profileDropdownList.classList.add("show");
  }

  document.addEventListener("click", (e) => {
    if (!e.target.closest(".profile-picker")) {
      profileDropdownList.classList.remove("show");
    }
    if (!e.target.closest(".save-picker")) {
      saveDropdownList.classList.remove("show");
    }
  });

  const profilePicker = document.querySelector(".profile-picker");
  if (profilePicker) {
    profilePicker.addEventListener("click", (e) => {
      if (e.target.closest(".custom-dropdown-list")) return;
      e.stopPropagation();
      const wasOpen = profileDropdownList.classList.contains("show");
      closeAllDropdowns();
      if (!wasOpen) profileDropdownList.classList.add("show");
    });
  }

  const savePicker = document.querySelector(".save-picker");
  if (savePicker) {
    savePicker.addEventListener("click", (e) => {
      if (e.target.closest(".dropdown-list")) return;
      e.stopPropagation();
      if (!window.selectedProfilePath) return;
      const wasOpen = saveDropdownList.classList.contains("show");
      closeAllDropdowns();
      if (!wasOpen) saveDropdownList.classList.add("show");
    });
  }

  // -----------------------------
  // PROFILE SCAN
  // -----------------------------
  scanBtn?.addEventListener("click", async () => {
    await scanProfiles({ saveToBackend: true, showToasts: true });
  });

  // -----------------------------
  // PROFILE LADEN
  // ----------------------------- 
  async function loadSelectedProfile() {
    if (!window.selectedProfilePath) return;

    try {
      profileStatus.textContent = "Loading profile...";

      // Clear old saves
      saveDropdownList.innerHTML = "";
      window.selectedSavePath = null;
      window.currentSavePath = null;
      saveNameDisplay.textContent = "Select a save";

      await invoke("load_profile", { profilePath: window.selectedProfilePath });

      // Scan saves for this profile
      await scanSavesForProfile();

      // Load configs (independent of save)
      try {
        await loadBaseConfig();
        await loadProfileSaveConfig();
      } catch (e) { 
        console.warn("Config load warning:", e); 
      }

      // Clear state (no save loaded yet)
      window.currentProfileData = {};
      window.currentQuicksaveData = {};
      window.allTrucks = [];
      window.playerTruck = null;
      window.allTrailers = [];
      window.playerTrailer = null;

      // Update Icons
      const cached = JSON.parse(localStorage.getItem("ets2_profiles_cache") || "[]");
      const profileInfo = cached.find(p => p.path === window.selectedProfilePath);
      
      window.selectedProfileHasAvatar = !!(profileInfo && profileInfo.avatar);
      const iconSrc = resolveProfileIcon(profileInfo);
      
      const footerIcon = document.getElementById("activeProfileIcon");
      if (footerIcon) {
        footerIcon.src = iconSrc;
        if (window.selectedProfileHasAvatar) footerIcon.onerror = () => handleIconError(footerIcon);
      }
      
      const navIcon = document.querySelector(".nav-icon-profile");
      if (navIcon) {
        navIcon.src = iconSrc;
        if (window.selectedProfileHasAvatar) navIcon.onerror = () => handleIconError(navIcon);
      }

      profileStatus.textContent = "Profile loaded. Please select a save.";
      showToast("toasts.profile_loaded_select_save", {}, "info");
      loadTools(activeTab);
    } catch (err) {
      console.error(err);
      profileStatus.textContent = "Error loading profile";
      showToast("toasts.profile_load_error", {}, "error");
    }
  }

  async function scanSavesForProfile() {
    if (!window.selectedProfilePath) return;

    saveDropdownList.innerHTML = "";
    openSaveModalBtn.disabled = false;

    try {
      const saves = await invoke("find_profile_saves", {
        profilePath: window.selectedProfilePath,
      });

      const filteredSaves = saves.filter(
        (s) => s.success && s.kind !== "Invalid"
      );

      filteredSaves.sort((a, b) => {
        const fA = a.folder.toLowerCase();
        const fB = b.folder.toLowerCase();

        const getPriority = (folder) => {
          if (folder === 'quicksave') return 0;
          if (folder === 'autosave') return 1;
          return 2;
        };

        const pA = getPriority(fA);
        const pB = getPriority(fB);

        if (pA !== pB) return pA - pB;

        return fB.localeCompare(fA, undefined, { numeric: true });
      });

      filteredSaves.forEach((s) => {
        const item = document.createElement("div");
        item.className = "dropdown-item";
        
        let displayName = s.name ?? s.folder;
        if (s.folder.toLowerCase() === 'quicksave') displayName = "~ Quicksave ~";
        else if (s.folder.toLowerCase() === 'autosave') displayName = "~ Autosave ~";
        else displayName = `${displayName} [${s.folder}]`;

        item.textContent = displayName;
        item.title = s.path;

        item.addEventListener("click", async () => {
          window.selectedSavePath = s.path;
          window.currentSavePath = s.path;
          saveNameDisplay.textContent = s.name ?? s.folder;
          saveDropdownList.classList.remove("show");

          await invoke("set_current_save", {
            savePath: s.path,
          });

          await loadSelectedSave();
        });

        saveDropdownList.appendChild(item);
      });
    } catch (e) {
      console.error(e);
      showToast("toasts.scan_saves_error", {}, "error");
    }
  }

  async function loadSelectedSave() {
    try {
      profileStatus.textContent = "Loading save...";

      await loadProfileData();
      await loadQuicksave();
      await loadProfileSaveConfig();
      await loadBaseConfig();
      await loadAllTrucks();
      await loadAllTrailers();

      updateUIWithCurrentQuicksave();

      profileStatus.textContent = "Save loaded successfully";
      showToast("toasts.save_loaded_success", {}, "success");
      loadTools(activeTab);
    } catch (e) {
      console.error(e);
      showToast("toasts.save_load_error", {}, "error");
    }
  }

  async function loadProfileData() {
    try {
      window.currentProfileData = await invoke("read_all_save_data");
    } catch (e) {
      console.error("Failed to load profile data:", e);
      throw e;
    }
  }

  async function loadQuicksave() {
    try {
      window.currentQuicksaveData = await invoke("quicksave_game_info");
    } catch (e) {
      console.error("Failed to load quicksave:", e);
      throw e;
    }
  }

  async function loadProfileSaveConfig() {
    try {
      window.readSaveGameConfig = await invoke("read_save_config", {
        profilePath: window.selectedProfilePath,
      });
    } catch (e) {
      console.error("Failed to load save config:", e);
      throw e;
    }
  }

  async function loadBaseConfig() {
    try {
      window.baseConfig = await invoke("read_base_config");
    } catch (e) {
      console.error("Failed to load base config:", e);
      throw e;
    }
  }

  async function loadAllTrucks() {
    try {
      window.playerTruck = await invoke("get_player_truck", {
        profilePath: window.selectedProfilePath,
      });
      window.allTrucks = [window.playerTruck];
    } catch (e) {
      console.error("Failed to load trucks:", e);
      window.playerTruck = null;
      window.allTrucks = [];
    }
  }

  async function loadAllTrailers() {
  try {
    // ← CHANGED: Now expecting Option<ParsedTrailer> from Rust
    const trailer = await invoke("get_player_trailer", {
      profilePath: window.selectedProfilePath,
    });
    
    // ← CHANGED: Check if trailer exists (could be null if player has no trailer)
    if (trailer) {
      window.playerTrailer = trailer;
      window.allTrailers = [trailer];
      console.log("✓ Player trailer loaded successfully");
    } else {
      // ← CHANGED: This is normal - player just doesn't have a trailer attached
      window.playerTrailer = null;
      window.allTrailers = [];
      console.log("ℹ Player has no trailer attached (this is normal)");
    }
  } catch (e) {
    // ← This should only catch actual errors now (not "no trailer" situations)
    console.error("Failed to load trailers:", e);
    window.playerTrailer = null;
    window.allTrailers = [];
  }
}

  // -----------------------------
  // SAVE MONEY / XP
  // -----------------------------
  moneyBtn?.addEventListener("click", async () => {
    try {
      const amount = Number(document.querySelector("#money-input").value);
      editStatus.textContent = "Saving…";
      await invoke("edit_money", { amount });
      editStatus.textContent = "Money saved!";
      showToast("toasts.money_saved_success", {}, "success");
      await loadProfileData();
      updateUIWithCurrentQuicksave();
      loadTools(activeTab);
    } catch (err) {
      console.error(err);
      editStatus.textContent = "Error saving money";
      showToast("toasts.money_save_error", {}, "error");
    }
  });

  levelBtn?.addEventListener("click", async () => {
    try {
      const xp = Number(document.querySelector("#level-input").value);
      editStatus.textContent = "Saving…";
      await invoke("edit_level", { xp });
      editStatus.textContent = "XP saved!";
      showToast("toasts.xp_saved_success", {}, "success");
      await loadProfileData();
      updateUIWithCurrentQuicksave();
      loadTools(activeTab);
    } catch (err) {
      console.error(err);
      editStatus.textContent = "Error saving XP";
      showToast("toasts.xp_save_error", {}, "error");
    }
  });

  // -----------------------------
  // EXTERNE LINKS
  // -----------------------------
  youtubeBtn?.addEventListener("click", () =>
    openUrl("https://www.youtube.com/@xLieferant")
  );
  patreonBtn?.addEventListener("click", () =>
    openUrl("https://www.patreon.com/cw/xLieferant")
  );
  githubBtn?.addEventListener("click", () =>
    openUrl("https://github.com/xLieferant/Save-Edit-Tool")
  );

  // -----------------------------
  // EXPOSE GLOBAL FUNCTIONS
  // -----------------------------
  window.loadQuicksave = loadQuicksave;
  window.loadProfileData = loadProfileData;
  window.loadProfileSaveConfig = loadProfileSaveConfig;
  window.loadBaseConfig = loadBaseConfig;
  window.loadAllTrucks = loadAllTrucks;
  window.loadAllTrailers = loadAllTrailers;

  // -----------------------------
  // CLONE PROFILE LOGIC
  // -----------------------------
  const cloneBtn = document.getElementById("cloneProfileBtn");
  cloneBtn?.addEventListener("click", async () => {
    if (!window.selectedProfilePath) {
      showToast("toasts.no_profile_selected", {}, "warning");
      return;
    }

    const choice = await openModalMulti("Manage Profile", [
      {
        type: "dropdown",
        id: "action",
        label: "Action",
        value: "Duplicate",
        options: ["Duplicate", "Rename"],
      },
    ]);

    if (!choice) return;

    switch (choice?.action) {
      case "Duplicate":
        openCloneProfileModal();
        break;

      case "Rename":
        await handleProfileRename();
        break;
    }
  });

  async function handleProfileRename() {
    const currentName = profileNameDisplay.textContent;
    const newName = await openModalText("Rename Profile", "New Name", currentName);

    if (newName && newName.trim() !== "" && newName !== currentName) {
      try {
        const newPath = await invoke("profile_rename", { newName: newName.trim() });
        showToast("toasts.profile_renamed_success", {}, "success");
        
        window.selectedProfilePath = newPath;
        profileNameDisplay.textContent = newName.trim();
        await scanProfiles({ saveToBackend: true, showToasts: false });
        await loadSelectedProfile();
      } catch (err) {
        console.error("Rename failed:", err);
        showToast("toasts.profile_rename_error", { error: err.toString() }, "error");
      }
    }
  }

  async function handleCopyControls() {
    if (!window.selectedProfilePath) {
      showToast("toasts.no_source_profile_selected", {}, "warning");
      return;
    }

    try {
      const profiles = await invoke("find_ets2_profiles");

      const sourcePath = window.selectedProfilePath;
      const targets = profiles.filter(
          (p) => p.success && p.path !== sourcePath
      );

      if (targets.length === 0) {
        showToast("toasts.no_other_profiles", {}, "warning");
        return;
      }

      const options = targets.map(
          (p) => `${p.name} [${p.path}]`
      );

      const res = await openModalMulti("Copy Controls", [
        {
          type: "dropdown",
          id: "target",
          label: "Target Profile",
          value: options[0],
          options: options,
        },
      ]);

      if (!res || !res.target) return;

      const selectedProfile = targets.find(
          (p) => `${p.name} [${p.path}]` === res.target
      );

      if (!selectedProfile) {
        showToast("toasts.invalid_profile_selected", {}, "error");
        return;
      }

      showToast("toasts.copying_controls", {}, "info");

      const msg = await invoke("copy_profile_controls", {
        sourceProfilePath: sourcePath,
        targetProfilePath: selectedProfile.path,
      });

      // Backend currently returns a hardcoded string, but we can try to translate it or override
      showToast("toasts.copy_controls_success", {}, "success");

    } catch (err) {
      console.error("Copy controls failed:", err);
      showToast("toasts.copy_controls_error", {}, "error");
    }
  }
  window.handleCopyControls = handleCopyControls;

  // -----------------------------
  // MOVE MODS LOGIC
  // -----------------------------
  async function handleMoveMods() {
    if (!window.selectedProfilePath) {
      showToast("toasts.no_source_profile_selected", {}, "warning");
      return;
    }

    try {
      const profiles = await invoke("find_ets2_profiles");
      
      const currentPath = window.selectedProfilePath;
      const targets = profiles.filter(p => p.success && p.path !== currentPath);

      if (targets.length === 0) {
        showToast("toasts.no_other_valid_profiles", {}, "warning");
        return;
      }

      const options = targets.map(p => `${p.name} [${p.path}]`);

      const res = await openModalMulti("Move Mods", [
        {
          type: "dropdown",
          id: "target",
          label: "Target Profile",
          value: options[0],
          options: options,
        },
      ]);

      if (!res || !res.target) return;

      const selectedStr = res.target;
      const selectedProfile = targets.find(p => `${p.name} [${p.path}]` === selectedStr);

      if (!selectedProfile) {
        showToast("toasts.invalid_profile_selection", {}, "error");
        return;
      }

      showToast("toasts.moving_mods_wait", {}, "info");

      const resultMsg = await invoke("copy_mods_to_profile", {
        targetProfilePath: selectedProfile.path,
      });

      // resultMsg contains a dynamic count from backend, but we use a key with placeholder
      // Extract number from "Erfolgreich X Mods übertragen." or "Successfully transferred X mods."
      const countMatch = resultMsg.match(/\d+/);
      const count = countMatch ? countMatch[0] : "?";
      
      showToast("toasts.move_mods_success", { count }, "success");

    } catch (err) {
      console.error("Move mods error:", err);
      showToast("toasts.move_mods_error", {}, "error");
    }
  }
  window.handleMoveMods = handleMoveMods;

  // -----------------------------
  // PROFILE SCAN (AUTO & CACHE)
  // -----------------------------
  async function scanProfiles({
    saveToBackend = true,
    showToasts = true,
  } = {}) {
    profileStatus.textContent = "Scanning profiles...";
    profileDropdownList.innerHTML = "";

    try {
      // Get current game to update UI
      try {
        const game = await invoke("get_selected_game");
        const ets2Btn = document.getElementById("ets2Btn");
        const atsBtn = document.getElementById("atsBtn");
        
        if (game === "ats") {
          ets2Btn.classList.remove("active");
          ets2Btn.disabled = false;
          atsBtn.classList.add("active");
          atsBtn.disabled = true;
        } else {
          atsBtn.classList.remove("active");
          atsBtn.disabled = false;
          ets2Btn.classList.add("active");
          ets2Btn.disabled = true;
        }
      } catch (e) {
        console.warn("Could not sync game buttons:", e);
      }

      const profiles = await invoke("find_ets2_profiles");
      profileStatus.textContent = `${profiles.length} profiles found`;
      if (showToasts) showToast("toasts.profiles_found", {}, "success");

      profiles.forEach((p) => {
        if (!p.success) return;
        const item = document.createElement("div");
        item.className = "dropdown-item";

        // Icon
        const img = document.createElement("img");
        img.src = resolveProfileIcon(p);
        img.className = "profile-icon-dropdown";
        if (p.avatar) {
          img.dataset.hasAvatar = "true";
          img.onerror = () => handleIconError(img);
        }

        // Text
        const textSpan = document.createElement("span");
        textSpan.textContent = p.name;

        item.appendChild(img);
        item.appendChild(textSpan);

        item.addEventListener("click", async () => {
          window.selectedProfilePath = p.path;
          profileNameDisplay.textContent = p.name;
          profileDropdownList.classList.remove("show");

          localStorage.setItem("ets2_last_profile", p.path);
          try {
            await invoke("save_last_profile", {
              profilePath: p.path,
            });
          } catch (e) {
            console.warn("save_last_profile failed", e);
          }

          await loadSelectedProfile();
        });

        profileDropdownList.appendChild(item);
      });

      try {
        localStorage.setItem("ets2_profiles_cache", JSON.stringify(profiles));
        if (saveToBackend) {
          const toSave = profiles.map((p) => ({
            path: p.path,
            name: p.name ?? null,
            avatar: p.avatar ?? null,
            success: !!p.success,
            message: p.message ?? null,
          }));
          await invoke("save_profiles_cache", { profiles: toSave });
        }
      } catch (e) {
        console.warn("Could not save profile cache:", e);
      }

      let last = null;
      try {
        const remoteLast = await invoke("read_last_profile");
        if (remoteLast) last = remoteLast;
      } catch (e) {
        // ignore
      }
      if (!last) {
        last = localStorage.getItem("ets2_last_profile");
      }

      if (last) {
        const matched = profiles.find((p) => p.path === last && p.success);
        if (matched && matched.success) {
          window.selectedProfilePath = matched.path;
          profileNameDisplay.textContent = matched.name ?? "Unknown";
          await loadSelectedProfile();
          return;
        } else {
          window.selectedProfilePath = last;
          profileNameDisplay.textContent = "Select Profile";
          try {
            await loadSelectedProfile();
            return;
          } catch (e) {
            console.warn("Autoload of last profile failed", e);
          }
        }
      }
    } catch (err) {
      console.error(err);
      profileStatus.textContent = "Scan failed";
      showToast("toasts.no_profiles_found", {}, "error");
    }
  }

  // -----------------------------
  // AUTO-SCAN ON STARTUP
  // -----------------------------
  (async function autoScanOnStartup() {
    try {
      const cached = await invoke("read_profiles_cache");
      if (cached && cached.length) {
        profileDropdownList.innerHTML = "";
        cached.forEach((p) => {
          if (!p.success) return;
          const item = document.createElement("div");
          item.className = "dropdown-item";
          
          // Icon
          const img = document.createElement("img");
          img.src = resolveProfileIcon(p); 
          img.className = "profile-icon-dropdown";
          if (p.avatar) {
            img.dataset.hasAvatar = "true";
            img.onerror = () => handleIconError(img);
          }
          
          // Text
          const textSpan = document.createElement("span");
          textSpan.textContent = p.name;

          item.appendChild(img);
          item.appendChild(textSpan);

          item.addEventListener("click", async () => {
            window.selectedProfilePath = p.path;
            profileNameDisplay.textContent = p.name;
            profileDropdownList.classList.remove("show");
            localStorage.setItem("ets2_last_profile", p.path);
            try {
              await invoke("save_last_profile", {
                profilePath: p.path,
              });
            } catch (e) {}
            await loadSelectedProfile();
          });
          profileDropdownList.appendChild(item);
        });

        try {
          const last = await invoke("read_last_profile");
          if (last) {
            window.selectedProfilePath = last;
            profileNameDisplay.textContent = "loading last profile...";
            await loadSelectedProfile();
            setTimeout(
              () => scanProfiles({ saveToBackend: true, showToasts: false }),
              500
            );
            return;
          }
        } catch (e) {
          // ignore
        }
      }
    } catch (e) {
      // no cache available
    }

    await scanProfiles({ saveToBackend: true, showToasts: true });
  })();
  // -----------------------------
// LANGUAGE PICKER
// -----------------------------
async function showLanguagePicker() { // #FIXME <-- Remove this code, we're using a diffrent Modal in tools.js! 
  try {
    const languages = await invoke('get_available_languages_command');
    const currentLang = await invoke('get_current_language_command');
    
    const modal = document.createElement('div');    
    document.body.appendChild(modal);
    
    const options = modal.querySelectorAll('.language-option');
    options.forEach(option => {
      option.addEventListener('click', async () => {
        const selectedLang = option.dataset.lang;
        
        if (selectedLang !== currentLang) {
          try {
            const message = await invoke('set_language_command', { 
              language: selectedLang 
            });
            
            showToast(message, "success");
            modal.remove();
            
            // Translate the UI again
            await translateUI();

          } catch (error) {
            showToast("toasts.generic_error_prefix", { error: error.toString() }, "error");
          }
        } else {
          modal.remove();
        }
      });
    });
    
    modal.querySelector('.close-modal').addEventListener('click', () => {
      modal.remove();
    });
    
    modal.addEventListener('click', (e) => {
      if (e.target === modal) {
        modal.remove();
      }
    });
    
  } catch (error) {
    showToast("toasts.load_languages_error", { error: error.toString() }, "error");
  }
}

// Helper function to get translations in JavaScript
async function t(key, params = {}) {
  try {
    let text = await invoke('translate_command', { key });

    // einfache Platzhalter-Ersetzung
    for (const [k, v] of Object.entries(params)) {
      text = text.replaceAll(`{${k}}`, String(v));
    }

    return text;
  } catch (error) {
    console.error('Translation error:', error);
    return key;
  }
}

// Function to translate all elements with data-translate attribute
async function translateUI() {
  const elements = document.querySelectorAll('[data-translate]');
  for (const el of elements) {
    const key = el.getAttribute('data-translate');
    el.textContent = await t(key);
  }
}

// Make functions globally available
window.showLanguagePicker = showLanguagePicker;
window.t = t;
window.translateUI = translateUI; // Make it global so you can call it from anywhere
});
