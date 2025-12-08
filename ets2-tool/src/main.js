const { invoke } = window.__TAURI__.core;

document.addEventListener("DOMContentLoaded", () => {
  /* -------------------------------------------------------------------------- */
  /*                               DOM ELEMENTE                                 */
  /* -------------------------------------------------------------------------- */

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
    profileStatus.textContent = "Scanning profiles...";
    profileDropdownList.innerHTML = "";

    try {
      const profiles = await invoke("find_ets2_profiles");
      profileStatus.textContent = `${profiles.length} profiles found`;

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
    } catch (e) {
      profileStatus.textContent = "Scan failed";
      console.error(e);
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

    profileStatus.textContent = "Loading profile...";

    try {
      await invoke("load_profile", { profilePath: selectedProfilePath });

      // Nacheinander alle relevanten Daten laden
      await loadProfileData();
      await loadQuicksave();
      await loadProfileSaveConfig();
      await loadBaseConfig();
      await loadAllTrucks();
      await parseTrucksFromSii();

      profileStatus.textContent = "Profile loaded";
      loadTools(activeTab);
    } catch (err) {
      console.error("Profile loading failed", err);
      profileStatus.textContent = "Error loading profile";
    }
  }

  /* -------------------------------------------------------------------------- */
  /*                       EINZELNE LADEN-FUNKTIONEN                             */
  /* -------------------------------------------------------------------------- */

  async function loadProfileData() {
    try {
      const data = await invoke("read_all_save_data");
      window.currentProfileData = data;

      if (moneyDisplay) moneyDisplay.textContent = `Geld: ${data.money.toLocaleString()} €`;
      if (xpDisplay) xpDisplay.textContent = `XP: ${data.xp.toLocaleString()}`;

    } catch (err) {
      console.error("Error profile data", err);
    }
  }

  async function loadQuicksave() {
    try {
      const data = await invoke("quicksave_game_info");
      window.currentQuicksaveData = data;
    } catch (err) {
      console.error("Error quicksave", err);
    }
  }

  async function loadProfileSaveConfig() {
    try {
      const data = await invoke("read_save_config", {
        profilePath: selectedProfilePath
      });
      window.readSaveGameConfig = data;
    } catch (err) {
      console.error("Error save config", err);
    }
  }

  async function loadBaseConfig() {
    try {
      const cfg = await invoke("read_base_config");
      window.baseConfig = cfg;
    } catch (err) {
      console.error("Error base config", err);
    }
  }

  async function loadAllTrucks() {
    try {
      const trucks = await invoke("get_all_trucks", {
        profilePath: selectedProfilePath
      });
      window.allTrucks = trucks || [];
    } catch (err) {
      console.error("Error truck list", err);
    }
  }

    async function parseTrucksFromSii() {
    try {
      const parseTruck = await invoke("parse_trucks_from_sii", {
        profilePath: selectedProfilePath
      });
      window.parseTruckSii = parseTruck || [];
    } catch (err) {
      console.error("Error truck list", err);
    }
  }

  /* -------------------------------------------------------------------------- */
  /*                         SAVE-FUNKTIONEN (MONEY / XP)                        */
  /* -------------------------------------------------------------------------- */

  if (moneyBtn) {
    moneyBtn.addEventListener("click", async () => {
      const amount = Number(document.querySelector("#money-input").value);
      editStatus.textContent = "Saving…";

      await invoke("edit_money", { amount });

      editStatus.textContent = "Money saved!";
      await loadProfileData();
      loadTools(activeTab);
    });
  }

  if (levelBtn) {
    levelBtn.addEventListener("click", async () => {
      const xp = Number(document.querySelector("#level-input").value);
      editStatus.textContent = "Saving…";

      await invoke("edit_level", { xp });

      editStatus.textContent = "XP saved!";
      await loadProfileData();
      loadTools(activeTab);
    });
  }

});
