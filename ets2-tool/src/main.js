import { loadTools, activeTab } from "./app.js";
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
// DOM READY – ZENTRALE INIT
// -----------------------------
document.addEventListener("DOMContentLoaded", () => {
  console.log("[main.js] DOM vollständig geladen.");

  // -----------------------------
  // BASIS INIT
  // -----------------------------
  initVersionInfo();

  // Auto-Updater (verzögert, stabil)
  setTimeout(() => {
    checkUpdaterOnStartup(showToast);
  }, 3000);

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

  let selectedProfilePath = null;
  let selectedSavePath = null;
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

  window.applySetting = applySetting;

  // -----------------------------
  // DROPDOWN
  // -----------------------------
  function toggleProfileDropdown() {
    profileDropdownList.classList.toggle("show");
  }

  document.addEventListener("click", (e) => {
    if (!e.target.closest(".profile-picker")) {
      profileDropdownList.classList.remove("show");
    }
    if (!e.target.closest(".save-picker")) {
      saveDropdownList.classList.remove("show");
    }
  });

  openProfileModalBtn?.addEventListener("click", (e) => {
    e.stopPropagation();
    toggleProfileDropdown();
  });

  openSaveModalBtn?.addEventListener("click", (e) => {
    e.stopPropagation();
    if (!selectedProfilePath) return;
    saveDropdownList.classList.toggle("show");
  });

  // -----------------------------
  // PROFILE SCAN
  // -----------------------------
  scanBtn?.addEventListener("click", async () => {
    profileStatus.textContent = "Scanning profiles...";
    profileDropdownList.innerHTML = "";

    try {
      const profiles = await invoke("find_ets2_profiles");
      profileStatus.textContent = `${profiles.length} profiles found`;
      showToast("Profiles found!", "success");

      profiles.forEach((p) => {
        if (!p.success) return;

        const item = document.createElement("div");
        item.className = "dropdown-item";
        item.textContent = `${p.name} (${p.path})`;

        item.addEventListener("click", async () => {
          selectedProfilePath = p.path;
          profileNameDisplay.textContent = p.name;
          profileDropdownList.classList.remove("show");
          await invoke("switch_profile", { new_profile_path: selectedProfilePath });
          await loadSelectedProfile();
        });

        profileDropdownList.appendChild(item);
      });
    } catch (err) {
      console.error(err);
      profileStatus.textContent = "Scan fehlgeschlagen";
      showToast("No profiles found!", "error");
    }
  });

  // -----------------------------
  // PROFILE LADEN
  // -----------------------------
async function loadSelectedProfile() {
  if (!selectedProfilePath) return;

  try {
    profileStatus.textContent = "Loading profile...";

    // → Alte Saves entfernen
    saveDropdownList.innerHTML = "";
    selectedSavePath = null;
    window.currentSavePath = null;
    saveNameDisplay.textContent = "Select a save";

    await invoke("load_profile", { profilePath: selectedProfilePath });

    await loadProfileData();
    await loadQuicksave();
    await loadProfileSaveConfig();
    await loadBaseConfig();
    await loadAllTrucks();

    profileStatus.textContent = "Profile loaded";
    showToast("Profile successfully loaded!", "success");
    loadTools(activeTab);

    // Scanne die neuen Saves
    await scanSavesForProfile();

  } catch (err) {
    console.error(err);
    profileStatus.textContent = "Error loading profile";
    showToast("Profile was not loaded!", "error");
  }
}

  async function scanSavesForProfile() {
  if (!selectedProfilePath) return;

  saveDropdownList.innerHTML = "";
  saveDropdownList.classList.add("show");
  openSaveModalBtn.disabled = false;

  try {
    const saves = await invoke("find_profile_saves", {
      profilePath: selectedProfilePath,
    });

    // 🔹 Nur gültige Saves (Autosave, Quicksave, nummerierte Manual)
    const filteredSaves = saves.filter(
      (s) => s.success && s.kind !== "Invalid"
    );

    // 🔹 Optional: sortieren (Autosave, Quicksave, dann nummeriert)
    filteredSaves.sort((a, b) => {
      const order = { "autosave": 0, "quicksave": 1 };
      const ka = order[a.folder.toLowerCase()] ?? 2;
      const kb = order[b.folder.toLowerCase()] ?? 2;
      if (ka !== kb) return ka - kb;
      return a.folder.localeCompare(b.folder, undefined, { numeric: true });
    });

    filteredSaves.forEach((s) => {
      const item = document.createElement("div");
      item.className = "dropdown-item";
      item.textContent = s.name ?? s.folder;

      item.addEventListener("click", async () => {
        selectedSavePath = s.path;
        window.currentSavePath = s.path;
        saveNameDisplay.textContent = s.name ?? s.folder;
        saveDropdownList.classList.remove("show");

        await invoke("set_current_save" , {
          savePath: s.path,
        });

        await loadSelectedSave();
      });

      saveDropdownList.appendChild(item);
    });
  } catch (e) {
    console.error(e);
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

      profileStatus.textContent = "Save loaded";
      showToast("Save loaded!", "success");
    } catch (e) {
      console.error(e);
      showToast("Failed to load save", "error");
    }
  }


  async function loadProfileData() {
    window.currentProfileData = await invoke("read_all_save_data");
  }

  async function loadQuicksave() {
    window.currentQuicksaveData = await invoke("quicksave_game_info");
  }

  async function loadProfileSaveConfig() {
    window.readSaveGameConfig = await invoke("read_save_config", {
      profilePath: selectedProfilePath,
    });
  }

  async function loadBaseConfig() {
    window.baseConfig = await invoke("read_base_config");
  }

  async function loadAllTrucks() {
    window.playerTruck = await invoke("get_player_truck", {
      profilePath: selectedProfilePath,
    });
    window.allTrucks = [window.playerTruck];
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
      await loadProfileData();
      loadTools(activeTab);
    } catch (err) {
      console.error(err);
      editStatus.textContent = "Error saving money";
    }
  });

  levelBtn?.addEventListener("click", async () => {
    try {
      const xp = Number(document.querySelector("#level-input").value);
      editStatus.textContent = "Saving…";
      await invoke("edit_level", { xp });
      editStatus.textContent = "XP saved!";
      await loadProfileData();
      loadTools(activeTab);
    } catch (err) {
      console.error(err);
      editStatus.textContent = "Error saving XP";
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

      // Fill dropdown
      profiles.forEach((p) => {
        if (!p.success) return;
        const item = document.createElement("div");
        item.className = "dropdown-item";
        item.textContent = `${p.name} (${p.path})`;

        item.addEventListener("click", async () => {
          selectedProfilePath = p.path;
          profileNameDisplay.textContent = p.name;
          profileDropdownList.classList.remove("show");

          // Persist last profile (localStorage + backend)
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

      // Save cache to localStorage and backend cache
      try {
        localStorage.setItem("ets2_profiles_cache", JSON.stringify(profiles));
        if (saveToBackend) {
          // Convert shapes to backend's expected shape: { path, name, success, message }
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

      // Auto-load last profile if present
      // Priority: backend last_profile -> localStorage last_profile -> none
      let last = null;
      try {
        const remoteLast = await invoke("read_last_profile");
        if (remoteLast) last = remoteLast;
      } catch (e) {
        // ignore: backend may not have been created yet
      }
      if (!last) {
        last = localStorage.getItem("ets2_last_profile");
      }

      if (last) {
        // try to find matching profile in this scan
        const matched = profiles.find((p) => p.path === last);
        if (matched && matched.success) {
          selectedProfilePath = matched.path;
          profileNameDisplay.textContent = matched.name ?? "Unknown";
          // load without further user action
          await loadSelectedProfile();
          return;
        } else {
          // fallback: try to load last path directly (may still work)
          selectedProfilePath = last;
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
      profileStatus.textContent = "Scan fehlgeschlagen";
      showToast("No profiles found!", "error");
    }
  }

  // replace old scan button handler with:
  scanBtn?.addEventListener("click", async () => {
    await scanProfiles({ saveToBackend: true, showToasts: true });
  });

  // -----------------------------
  // AUTO-SCAN ON STARTUP
  // -----------------------------
  (async function autoScanOnStartup() {
    // Try to read backend cache first so we can show something instantly
    try {
      const cached = await invoke("read_profiles_cache");
      if (cached && cached.length) {
        // populate dropdown from cache first (fast)
        profileDropdownList.innerHTML = "";
        cached.forEach((p) => {
          if (!p.success) return;
          const item = document.createElement("div");
          item.className = "dropdown-item";
          item.textContent = `${p.name} (${p.path})`;
          item.addEventListener("click", async () => {
            selectedProfilePath = p.path;
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

        // If there is a last_profile saved, try to load it
        try {
          const last = await invoke("read_last_profile");
          if (last) {
            selectedProfilePath = last;
            profileNameDisplay.textContent = "loading last profile...";
            await loadSelectedProfile();
            // After we've loaded, also perform background scan to refresh cache new profiles
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
      // no cache available — continue to scanning
    }

    // If we reach here: no cache/last found -> do a scan now (this will create the cache)
    await scanProfiles({ saveToBackend: true, showToasts: true });
  })();
});
