const { invoke } = window.__TAURI__.core;

document.addEventListener("DOMContentLoaded", () => {
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
  /*                               PROFILE SCANNEN                              */
  /* -------------------------------------------------------------------------- */

  scanBtn.addEventListener("click", async () => {
    profileStatus.textContent = "Scanning profiles...";
    profileDropdownList.innerHTML = "";

    const profiles = await invoke("find_ets2_profiles");

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

    profileStatus.textContent = `${profiles.length} profiles found`;
  });

  /* -------------------------------------------------------------------------- */
  /*                           PROFIL LADEN & LOGIK                             */
  /* -------------------------------------------------------------------------- */

  async function loadSelectedProfile() {
    if (!selectedProfilePath) {
      profileStatus.textContent = "No profile selected!";
      return;
    }

    profileStatus.textContent = "Loading autosave/info.sii...";
    const result = await invoke("load_profile", {
      profilePath: selectedProfilePath,
    });
    profileStatus.textContent = result;

    await updateAllDisplays();

    const saveConfig = await invoke("read_save_config", {
      profilePath: selectedProfilePath,
    });
    console.log("Profil-Config:", saveConfig);

    const globalConfig = await loadGlobalConfig();
    console.log("Globale Config geladen:", globalConfig);
  }

  /* -------------------------------------------------------------------------- */
  /*                              SAVE-DATEN ANZEIGEN                           */
  /* -------------------------------------------------------------------------- */

  async function updateAllDisplays() {
    try {
      const data = await invoke("read_all_save_data");
      window.currentProfileData = data;

      if (moneyDisplay)
        moneyDisplay.textContent = `Geld: ${data.money.toLocaleString()} €`;

      if (xpDisplay) xpDisplay.textContent = `XP: ${data.xp.toLocaleString()}`;

      loadTools(activeTab);
    } catch (error) {
      console.error(error);
    }
  }

  /* -------------------------------------------------------------------------- */
  /*                       GLOBALE BASEGAME CONFIG (Optimiert)                  */
  /* -------------------------------------------------------------------------- */

  // Ruft die globale config.cfg aus Rust ab
  async function getGlobalConfig() {
    try {
      return await invoke("read_base_config");
    } catch (error) {
      console.error("Fehler beim Auslesen der globalen Config:", error);
      return null;
    }
  }

  // Lädt globale Config einmal und speichert sie global ab
  async function loadGlobalConfig() {
    try {
      const cfg = await getGlobalConfig();
      window.baseConfig = cfg;
      return cfg;
    } catch (err) {
      console.error("Fehler beim GlobalConfig:", err);
      return null;
    }
  }

/* -------------------------------------------------------------------------- */
/*                          QUICKSAVE GAME DATA (Optimiert)                   */
/* -------------------------------------------------------------------------- */

/**
 * Ruft die Quicksave-Daten aus Rust ab
 */
async function getQuicksaveGame() {
  try {
    return await invoke("quicksave_game_info");
  } catch (error) {
    console.error("Fehler beim Auslesen der Quicksave-Daten:", error);
    return null;
  }
}

/**
 * Lädt Quicksave-Daten einmal und speichert sie global
 */
async function loadQuicksaveGame() {
  try {
    const data = await getQuicksaveGame();
    window.quicksaveData = data;
    return data;
  } catch (err) {
    console.error("Fehler beim Laden der Quicksave-Daten:", err);
    return null;
  }
}

// Beispielnutzung
loadQuicksaveGame().then(data => {
  if (data) {
    console.log("Quicksave geladen:", data);
  }
});


  /* -------------------------------------------------------------------------- */
  /*                         SAVE FUNKTIONEN (MONEY / XP)                       */
  /* -------------------------------------------------------------------------- */

  // Save money
  if (moneyBtn) {
    moneyBtn.addEventListener("click", async () => {
      const amount = Number(document.querySelector("#money-input").value);
      editStatus.textContent = "Saving...";

      await invoke("edit_money", { amount });

      editStatus.textContent = "Money saved!";
      await updateAllDisplays();
    });
  }

  // Save XP/Level
  if (levelBtn) {
    levelBtn.addEventListener("click", async () => {
      const xp = Number(document.querySelector("#level-input").value);
      editStatus.textContent = "Saving...";

      await invoke("edit_level", { xp });

      editStatus.textContent = "XP saved!";
      await updateAllDisplays();
    });
  }

  // Global config optional direkt laden
  // loadGlobalConfig();
});
