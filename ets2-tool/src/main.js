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
    case "success": return "✔";
    case "error":   return "✖";
    case "warning": return "⚠";
    case "info":
    default:        return "ℹ";
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

  const moneyBtn = document.querySelector("#save-money-btn");
  const levelBtn = document.querySelector("#save-level-btn");
  const editStatus = document.querySelector("#edit-status");

  const youtubeBtn = document.querySelector("#youtubeBtn");
  const patreonBtn = document.querySelector("#patreonBtn");
  const githubBtn = document.querySelector("#githubBtn");

  let selectedProfilePath = null;

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
  });

  openProfileModalBtn?.addEventListener("click", (e) => {
    e.stopPropagation();
    toggleProfileDropdown();
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
      await invoke("load_profile", { profilePath: selectedProfilePath });

      await loadProfileData();
      await loadQuicksave();
      await loadProfileSaveConfig();
      await loadBaseConfig();
      await loadAllTrucks();

      profileStatus.textContent = "Profile loaded";
      showToast("Profile successfully loaded!", "success");
      loadTools(activeTab);
    } catch (err) {
      console.error(err);
      profileStatus.textContent = "Error loading profile";
      showToast("Profile was not loaded!", "error");
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
});
