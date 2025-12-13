import { loadTools, activeTab } from "./app.js";

console.log("[main.js] Skript gestartet.");

const { invoke } = window.__TAURI__.core;
window.invoke = invoke; // Mache invoke global verfügbar, damit es in tools.js funktioniert

document.addEventListener("DOMContentLoaded", () => {
  console.log("[main.js] DOM vollständig geladen.");
  /* -------------------------------------------------------------------------- */
  /*                               DOM ELEMENTE                                 */
  /* -------------------------------------------------------------------------- */
  console.log("[main.js] Lade DOM-Elemente.");
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

  let selectedProfilePath = null;

  window.currentProfileData = {};
  window.currentQuicksaveData = {};
  window.readSaveGameConfig = {};
  window.baseConfig = {};
  window.allTrucks = [];
  window.parseTruckSii = [];
  window.playerTruck = null; // <-- Player Truck automatisch

  // Mache Ladefunktionen global verfügbar, damit sie in tools.js funktionieren
  window.loadProfileData = loadProfileData;
  window.loadQuicksave = loadQuicksave;
  window.loadProfileSaveConfig = loadProfileSaveConfig;
  window.loadBaseConfig = loadBaseConfig;
  window.loadAllTrucks = loadAllTrucks;

  /* -------------------------------------------------------------------------- */
  /*                           DROPDOWN STEUERUNG                               */
  /* -------------------------------------------------------------------------- */
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

  /* -------------------------------------------------------------------------- */
  /*                           PROFILE SCANNEN                                  */
  /* -------------------------------------------------------------------------- */
  scanBtn.addEventListener("click", async () => {
    console.log("[main.js] 'Refresh' geklickt, starte Profil-Scan.");
    profileStatus.textContent = "Scanning profiles...";
    profileDropdownList.innerHTML = "";

    try {
      const profiles = await invoke("find_ets2_profiles");
      profileStatus.textContent = `${profiles.length} profiles found`;
      console.log(`[main.js] ${profiles.length} Profile gefunden.`);

      profiles.forEach((p) => {
        if (!p.success) return;

        const item = document.createElement("div");
        item.classList.add("dropdown-item");
        item.textContent = `${p.name} (${p.path})`;
        item.dataset.path = p.path;

        item.addEventListener("click", () => {
          console.log(`[main.js] Profil ausgewählt: ${p.name}`);
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
    }
  });

  /* -------------------------------------------------------------------------- */
  /*                        PROFIL + SAVEFILES LADEN                             */
  /* -------------------------------------------------------------------------- */
  async function loadSelectedProfile() {
    if (!selectedProfilePath) {
      profileStatus.textContent = "No profile selected!";
      return;
    }

    console.log("[main.js] Lade ausgewähltes Profil...");
    profileStatus.textContent = "Loading profile...";

    try {
      await invoke("load_profile", { profilePath: selectedProfilePath });

      await loadProfileData();
      await loadQuicksave();
      await loadProfileSaveConfig();
      await loadBaseConfig();
      await loadAllTrucks();

      profileStatus.textContent = "Profile loaded";
      console.log("[main.js] Profil vollständig geladen. Lade Tools.");
      loadTools(activeTab);
    } catch (err) {
      console.error("Laden des Profils fehlgeschlagen", err);
      profileStatus.textContent = "Error loading profile";
    }
  }

  /* -------------------------------------------------------------------------- */
  /*                       EINZELNE LADEN-FUNKTIONEN                             */
  /* -------------------------------------------------------------------------- */
  async function loadProfileData() {
    try {
      console.log("[main.js] Lade Profildaten (Geld, XP)...");
      const data = await invoke("read_all_save_data");
      window.currentProfileData = data;

      if (moneyDisplay)
        moneyDisplay.textContent = `Geld: ${data.money.toLocaleString()} €`;
      if (xpDisplay) xpDisplay.textContent = `XP: ${data.xp.toLocaleString()}`;
    } catch (err) {
      console.error("Fehler beim Laden der Profildaten", err);
    }
  }

  async function loadQuicksave() {
    try {
      console.log("[main.js] Lade Quicksave-Daten (Skills)...");
      const data = await invoke("quicksave_game_info");
      window.currentQuicksaveData = data;
    } catch (err) {
      console.error("Fehler beim Laden des Quicksaves", err);
    }
  }

  async function loadProfileSaveConfig() {
    try {
      console.log("[main.js] Lade Save-Config (z.B. Parking Doubles)...");
      const data = await invoke("read_save_config", {
        profilePath: selectedProfilePath,
      });
      window.readSaveGameConfig = data;
    } catch (err) {
      console.error("Error loading save config", err);
    }
  }

  async function loadBaseConfig() {
    try {
      console.log("[main.js] Lade Base-Config (z.B. Traffic, Dev Mode)...");
      const cfg = await invoke("read_base_config");
      window.baseConfig = cfg;
    } catch (err) {
      console.error("Fehler beim Laden der Base-Config", err);
    }
  }

  async function loadAllTrucks() {
    try {
      console.log("[main.js] Lade Truck-Daten...");
      if (!selectedProfilePath) return;

      const playerTruck = await invoke("get_player_truck", {
        profilePath: selectedProfilePath,
      });

      window.playerTruck = playerTruck; // Player Truck automatisch setzen
      window.allTrucks = [window.playerTruck]; // für Kompatibilität mit allen Trucks
      console.log("[main.js] Spieler-Truck geladen:", window.playerTruck);
    } catch (err) {
      console.error("Error loading trucks", err);
    }
  }

  // Hilfsfunktion: aktiven Truck holen (Standard: playerTruck)
  window.getActiveTruck = function () {
    return window.playerTruck || {};
  };

  /* -------------------------------------------------------------------------- */
  /*                         SAVE-FUNKTIONEN (MONEY / XP)                        */
  /* -------------------------------------------------------------------------- */
  if (moneyBtn) {
    moneyBtn.addEventListener("click", async () => {
      const amount = Number(document.querySelector("#money-input").value);
      console.log(`[main.js] Speichere Geld: ${amount}`);
      editStatus.textContent = "Saving…";

      try {
        await invoke("edit_money", { amount });
        editStatus.textContent = "Money saved!";
        await loadProfileData();
        loadTools(activeTab);
      } catch (err) {
        console.error("Fehler beim Speichern des Geldes", err);
        editStatus.textContent = "Error saving money";
      }
    });
  }

  if (levelBtn) {
    levelBtn.addEventListener("click", async () => {
      const xp = Number(document.querySelector("#level-input").value);
      console.log(`[main.js] Speichere XP: ${xp}`);
      editStatus.textContent = "Saving…";

      try {
        await invoke("edit_level", { xp });
        editStatus.textContent = "XP saved!";
        await loadProfileData();
        loadTools(activeTab);
      } catch (err) {
        console.error("Fehler beim Speichern der XP", err);
        editStatus.textContent = "Error saving XP";
      }
    });
  }
});
