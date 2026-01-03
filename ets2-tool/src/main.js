import { loadTools, activeTab, openCloneProfileModal, openModalMulti, openModalText } from "./app.js";
import { applySetting } from "./js/applySetting.js";
import { checkUpdaterOnStartup, manualUpdateCheck } from "./js/updater.js";

const { app } = window.__TAURI__;
const { openUrl } = window.__TAURI__.opener;
const { invoke } = window.__TAURI__.core;
window.invoke = invoke; // global verfügbar

// -----------------------------
// TOAST-FUNKTION
// -----------------------------
window.showToast = function (message, type = "info") {
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

      profileStatus.textContent = "Profile loaded. Please select a save.";
      showToast("Profile loaded. Please select a save game.", "info");
      loadTools(activeTab);
    } catch (err) {
      console.error(err);
      profileStatus.textContent = "Error loading profile";
      showToast("Profile could not be loaded!", "error");
    }
  }

  async function scanSavesForProfile() {
    if (!window.selectedProfilePath) return;

    saveDropdownList.innerHTML = "";
    saveDropdownList.classList.add("show");
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
      showToast("Could not scan saves", "error");
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
      showToast("Save loaded successfully!", "success");
      loadTools(activeTab);
    } catch (e) {
      console.error(e);
      showToast("Failed to load save", "error");
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
      window.playerTrailer = await invoke("get_player_trailer", {
        profilePath: window.selectedProfilePath,
      });
      window.allTrailers = [window.playerTrailer];
    } catch (e) {
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
      showToast("Money successfully saved!", "success");
      await loadProfileData();
      updateUIWithCurrentQuicksave();
      loadTools(activeTab);
    } catch (err) {
      console.error(err);
      editStatus.textContent = "Error saving money";
      showToast("Failed to save money", "error");
    }
  });

  levelBtn?.addEventListener("click", async () => {
    try {
      const xp = Number(document.querySelector("#level-input").value);
      editStatus.textContent = "Saving…";
      await invoke("edit_level", { xp });
      editStatus.textContent = "XP saved!";
      showToast("XP successfully saved!", "success");
      await loadProfileData();
      updateUIWithCurrentQuicksave();
      loadTools(activeTab);
    } catch (err) {
      console.error(err);
      editStatus.textContent = "Error saving XP";
      showToast("Failed to save XP", "error");
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
      showToast("No profile selected!", "warning");
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

    if (choice.action === "Duplicate") {
      openCloneProfileModal();
    } else if (choice.action === "Rename") {
      await handleProfileRename();
    }
  });

  async function handleProfileRename() {
    const currentName = profileNameDisplay.textContent;
    const newName = await openModalText("Rename Profile", "New Name", currentName);

    if (newName && newName.trim() !== "" && newName !== currentName) {
      try {
        const newPath = await invoke("profile_rename", { newName: newName.trim() });
        showToast("Profile renamed successfully!", "success");
        
        window.selectedProfilePath = newPath;
        profileNameDisplay.textContent = newName.trim();
        await scanProfiles({ saveToBackend: true, showToasts: false });
        await loadSelectedProfile();
      } catch (err) {
        console.error("Rename failed:", err);
        showToast(err.toString(), "error");
      }
    }
  }

  // -----------------------------
  // MOVE MODS LOGIC
  // -----------------------------
  async function handleMoveMods() {
    if (!window.selectedProfilePath) {
      showToast("No source profile selected!", "warning");
      return;
    }

    try {
      const profiles = await invoke("find_ets2_profiles");
      
      const currentPath = window.selectedProfilePath;
      const targets = profiles.filter(p => p.success && p.path !== currentPath);

      if (targets.length === 0) {
        showToast("No other valid profiles found.", "warning");
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
        showToast("Invalid profile selection.", "error");
        return;
      }

      showToast("Moving mods... please wait", "info");

      const resultMsg = await invoke("copy_mods_to_profile", {
        targetProfilePath: selectedProfile.path,
      });

      showToast(resultMsg, "success");

    } catch (err) {
      console.error("Move mods error:", err);
      showToast(typeof err === "string" ? err : "Failed to move mods.", "error");
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
      const profiles = await invoke("find_ets2_profiles");
      profileStatus.textContent = `${profiles.length} profiles found`;
      if (showToasts) showToast("Profiles found!", "success");

      profiles.forEach((p) => {
        if (!p.success) return;
        const item = document.createElement("div");
        item.className = "dropdown-item";
        item.textContent = `${p.name} (${p.path})`;

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
          profileNameDisplay.textContent = last;
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
      showToast("No profiles found!", "error");
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
          item.textContent = `${p.name} (${p.path})`;
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
});