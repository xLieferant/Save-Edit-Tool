import { loadTools, activeTab } from "./app.js";
import { applySetting } from "./js/applySetting.js";

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
    default:        return "U+0069";
  }
}


// -----------------------------
// APP VERSION HILFSFUNKTION
// -----------------------------
async function appVersion() {
  try {
    return await app.getVersion();
  } catch (err) {
    console.error("Version konnte nicht geladen werden:", err);
    return "0.0.0";
  }
}

// -----------------------------
// INIT VERSION INFO
// -----------------------------
async function initVersionInfo() {
  const version = await appVersion();
  const versionElement = document.querySelector(".version-info");
  if (versionElement) {
    versionElement.textContent = `v${version}`;
  }
}

// -----------------------------
// UPDATE CHECK
// -----------------------------
async function checkForUpdate() {
  showToast("Suche nach Updates …", "info");

  const currentVersion = await appVersion();
  const repo = "xLieferant/Save-Edit-Tool";

  try {
    const res = await fetch(`https://api.github.com/repos/${repo}/releases/latest`);
    const data = await res.json();
    const latestVersion = data.tag_name.replace(/^v/, "");

    const comparison = compareVersions(currentVersion, latestVersion);

    if (comparison >= 0) {
      showToast(
          comparison === 1
              ? "Dev-Version aktiv (neuer als Release)"
              : "Du hast die neueste Version",
          "success"
      );
    } else {
      showToast(`Update verfügbar: v${latestVersion}`, "warning");
      console.log("Update:", data.html_url);
    }
  } catch {
    showToast("Update-Check fehlgeschlagen", "error");
  }
}


/**
 * Vergleicht zwei SemVer-Strings.
 * Gibt 1 zurück, wenn v1 > v2
 * Gibt -1 zurück, wenn v1 < v2
 * Gibt 0 zurück, wenn v1 == v2
 */
function compareVersions(v1, v2) {
  const parts1 = v1.replace(/^v/, "").split('.').map(Number);
  const parts2 = v2.replace(/^v/, "").split('.').map(Number);
  for (let i = 0; i < Math.max(parts1.length, parts2.length); i++) {
    const p1 = parts1[i] || 0;
    const p2 = parts2[i] || 0;
    if (p1 > p2) return 1;
    if (p1 < p2) return -1;
  }
  return 0;
}


document.addEventListener("DOMContentLoaded", () => {
  const versionBtn = document.getElementById("versionBtn");
  versionBtn.addEventListener("click", checkForUpdate);

  // Optional: alle 10 Minuten automatisch prüfen
  setInterval(checkForUpdate, 10 * 60 * 1000);
});

// -----------------------------
// DOMContentLoaded
// -----------------------------
document.addEventListener("DOMContentLoaded", () => {
  console.log("[main.js] DOM vollständig geladen.");
  initVersionInfo();

  const scanBtn = document.querySelector("#refreshBtn");
  const profileNameDisplay = document.querySelector("#profileNameDisplay");
  const profileDropdownList = document.querySelector("#profileDropdownList");
  const openProfileModalBtn = document.querySelector("#openProfileModal");
  const profileStatus = document.querySelector("#profile-status");

  const moneyDisplay = document.querySelector("#moneyShow");
  const xpDisplay = document.querySelector("#xpShow");

  const moneyBtn = document.querySelector("#save-money-btn");
  const levelBtn = document.querySelector("#save-level-btn");
  const editStatus = document.querySelector("#edit-status");

  const youtubeBtn = document.querySelector("#youtubeBtn");
  const patreonBtn = document.querySelector("#patreonBtn");
  const githubBtn = document.querySelector("#githubBtn");
  const checkUpdateBtn = document.getElementById("checkUpdateBtn");

  let selectedProfilePath = null;

  window.currentProfileData = {};
  window.currentQuicksaveData = {};
  window.readSaveGameConfig = {};
  window.baseConfig = {};
  window.allTrucks = [];
  window.playerTruck = null;

  window.loadProfileData = loadProfileData;
  window.loadQuicksave = loadQuicksave;
  window.loadProfileSaveConfig = loadProfileSaveConfig;
  window.loadBaseConfig = loadBaseConfig;
  window.loadAllTrucks = loadAllTrucks;
  window.applySetting = applySetting;

  // -----------------------------
  // Dropdown-Steuerung
  // -----------------------------
  function toggleProfileDropdown() {
    profileDropdownList.classList.toggle("show");
  }

  document.addEventListener("click", (event) => {
    if (!event.target.closest(".profile-picker")) {
      profileDropdownList.classList.remove("show");
    }
  });

  openProfileModalBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    toggleProfileDropdown();
  });

  // -----------------------------
  // Profile scannen
  // -----------------------------
  scanBtn.addEventListener("click", async () => {
    profileStatus.textContent = "Scanning profiles...";
    profileDropdownList.innerHTML = "";

    try {
      const profiles = await invoke("find_ets2_profiles");
      profileStatus.textContent = `${profiles.length} profiles found`;
      showToast("Profiles found!", "success");

      profiles.forEach((p) => {
        if (!p.success) return;

        const item = document.createElement("div");
        item.classList.add("dropdown-item");
        item.textContent = `${p.name} (${p.path})`;
        item.dataset.path = p.path;

        item.addEventListener("click", () => {
          selectedProfilePath = p.path;
          profileNameDisplay.textContent = p.name;
          profileDropdownList.classList.remove("show");
          loadSelectedProfile();
        });

        profileDropdownList.appendChild(item);
      });
    } catch (err) {
      profileStatus.textContent = "Scan fehlgeschlagen";
      console.error(err);
      showToast("No profiles found!", "error");
    }
  });

  // -----------------------------
  // Profile laden
  // -----------------------------
  async function loadSelectedProfile() {
    if (!selectedProfilePath) {
      profileStatus.textContent = "No profile selected!";
      return;
    }

    profileStatus.textContent = "Loading profile...";

    try {
      await invoke("load_profile", { profilePath: selectedProfilePath });

      await loadProfileData();
      await loadQuicksave();
      await loadProfileSaveConfig();
      await loadBaseConfig();
      await loadAllTrucks();

      profileStatus.textContent = "Profile loaded";
      showToast("Profil succesfully loaded!", "success");
      loadTools(activeTab);
    } catch (err) {
      profileStatus.textContent = "Error loading profile";
      showToast("Profil was not loaded!", "error");
      console.error(err);
    }
  }

  // -----------------------------
  // Save-Funktionen
  // -----------------------------
  async function loadProfileData() {
    try {
      const data = await invoke("read_all_save_data");
      window.currentProfileData = data;
    } catch (err) {
      console.error(err);
    }
  }

  async function loadQuicksave() {
    try {
      const data = await invoke("quicksave_game_info");
      window.currentQuicksaveData = data;
    } catch (err) {
      console.error(err);
    }
  }

  async function loadProfileSaveConfig() {
    try {
      const data = await invoke("read_save_config", {
        profilePath: selectedProfilePath,
      });
      window.readSaveGameConfig = data;
    } catch (err) {
      console.error(err);
    }
  }

  async function loadBaseConfig() {
    try {
      const cfg = await invoke("read_base_config");
      window.baseConfig = cfg;
    } catch (err) {
      console.error(err);
    }
  }

  async function loadAllTrucks() {
    try {
      if (!selectedProfilePath) return;
      const playerTruck = await invoke("get_player_truck", {
        profilePath: selectedProfilePath,
      });
      window.playerTruck = playerTruck;
      window.allTrucks = [window.playerTruck];
    } catch (err) {
      console.error(err);
    }
  }

  window.getActiveTruck = function () {
    return window.playerTruck || {};
  };

  // -----------------------------
  // Save Money / XP
  // -----------------------------
  if (moneyBtn) {
    moneyBtn.addEventListener("click", async () => {
      const amount = Number(document.querySelector("#money-input").value);
      editStatus.textContent = "Saving…";
      try {
        await invoke("edit_money", { amount });
        editStatus.textContent = "Money saved!";
        await loadProfileData();
        loadTools(activeTab);
      } catch (err) {
        editStatus.textContent = "Error saving money";
        console.error(err);
      }
    });
  }

  if (levelBtn) {
    levelBtn.addEventListener("click", async () => {
      const xp = Number(document.querySelector("#level-input").value);
      editStatus.textContent = "Saving…";
      try {
        await invoke("edit_level", { xp });
        editStatus.textContent = "XP saved!";
        await loadProfileData();
        loadTools(activeTab);
      } catch (err) {
        editStatus.textContent = "Error saving XP";
        console.error(err);
      }
    });
  }

  // -----------------------------
  // License Plate Helper
  // -----------------------------
  window.extractPlateText = (raw) => {
    if (!raw) return "";
    return raw
      .replace(/<[^>]*>/g, "")
      .split("|")[0]
      .trim();
  };

  // -----------------------------
  // Links öffnen
  // -----------------------------

  youtubeBtn.addEventListener("click", async () => {
    try {
      await openUrl("https://www.youtube.com/@xLieferant");
    } catch (err) {
      console.error("Fehler beim Öffnen von YouTube:", err);
      alert("YouTube konnte nicht geöffnet werden.");
    }
  });
  patreonBtn.addEventListener("click", async () => {
    try {
      await openUrl("https://www.patreon.com/cw/xLieferant");
    } catch (err) {
      console.error("Fehler beim Öffnen von YouTube:", err);
      alert("YouTube konnte nicht geöffnet werden.");
    }
  });
  githubBtn.addEventListener("click", async () => {
    try {
      await openUrl("https://github.com/xLieferant/Save-Edit-Tool");
    } catch (err) {
      console.error("Fehler beim Öffnen von GitHub:", err);
      alert("GitHub konnte nicht geöffnet werden.");
    }
  });

  // -----------------------------
  // Update-Button
  // -----------------------------
  if (checkUpdateBtn) {
    checkUpdateBtn.addEventListener("click", checkForUpdate);
  }
});
