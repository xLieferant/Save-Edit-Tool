const { invoke } = window.__TAURI__.core;

document.addEventListener("DOMContentLoaded", () => {
  const scanBtn = document.querySelector("#refreshBtn");
  const profileSelect = document.querySelector("#profileName");
  const loadProfileBtn = document.querySelector("#load-profile-btn");
  const profileStatus = document.querySelector("#profile-status");

  const moneyDisplay = document.querySelector("#moneyShow");
  const xpDisplay = document.querySelector("#xpShow");   // FIXED!

  const moneyBtn = document.querySelector("#save-money-btn");
  const levelBtn = document.querySelector("#save-level-btn");
  const editStatus = document.querySelector("#edit-status");

  let selectedProfilePath = null;


  // --- PROFILE SCANNEN ---
  scanBtn.addEventListener("click", async () => {
    profileStatus.textContent = "Scanne Profile...";
    profileSelect.innerHTML = `<option>Bitte Profil wählen...</option>`;

    const profiles = await invoke("find_ets2_profiles");

    profiles.forEach((p) => {
      if (!p.success) return;

      const opt = document.createElement("option");
      opt.value = p.path;
      opt.textContent = `${p.name} (${p.path})`;
      profileSelect.appendChild(opt);
    });

    profileStatus.textContent = `${profiles.length} Profile gefunden`;
  });


  profileSelect.addEventListener("change", () => {
    selectedProfilePath = profileSelect.value;
  });


  // --- PROFIL LADEN ---
  loadProfileBtn.addEventListener("click", async () => {
    if (!selectedProfilePath) {
      profileStatus.textContent = "Kein Profil ausgewählt!";
      return;
    }

    profileStatus.textContent = "Lade autosave/info.sii...";
    const result = await invoke("load_profile", {
      profilePath: selectedProfilePath,
    });
    profileStatus.textContent = result;

    // Nach dem Laden Geld & XP aktualisieren
    await updateMoneyDisplay();
    await updateXpDisplay();
  });


  // --- GELD LESEN ---
  async function updateMoneyDisplay() {
    try {
      const money = await invoke("read_money");
      moneyDisplay.textContent = `Geld: ${money.toLocaleString()} €`;
    } catch (error) {
      moneyDisplay.textContent = `Fehler beim Laden: ${error}`;
    }
  }


  // --- XP LESEN ---
  async function updateXpDisplay() {
    try {
      const xp = await invoke("read_xp");
      xpDisplay.textContent = `Erfahrungspunkte: ${xp.toLocaleString()} XP`;
    } catch (error) {
      xpDisplay.textContent = `Fehler beim Laden: ${error}`;
    }
  }


  // --- GELD SPEICHERN ---
  moneyBtn.addEventListener("click", async () => {
    const amount = Number(document.querySelector("#money-input").value);
    editStatus.textContent = "Speichere...";

    await invoke("edit_money", { amount });

    editStatus.textContent = "Geld geändert!";
    await updateMoneyDisplay();
  });


  // --- LEVEL (XP) SPEICHERN ---
  levelBtn.addEventListener("click", async () => {
    const xp = Number(document.querySelector("#level-input").value);
    editStatus.textContent = "Speichere...";

    await invoke("edit_level", { xp });

    editStatus.textContent = "Level geändert!";
    await updateXpDisplay();
  });
});
