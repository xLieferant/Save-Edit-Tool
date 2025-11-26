const { invoke } = window.__TAURI__.core;

document.addEventListener("DOMContentLoaded", () => {
  const scanBtn = document.querySelector("#scan-profiles-btn");
  const profileSelect = document.querySelector("#profile-select");
  const loadProfileBtn = document.querySelector("#load-profile-btn");
  const profileStatus = document.querySelector("#profile-status");

  const moneyDisplay = document.querySelector("#moneyShow");

  const moneyBtn = document.querySelector("#save-money-btn");
  const levelBtn = document.querySelector("#save-level-btn");
  const editStatus = document.querySelector("#edit-status");

  let selectedProfilePath = null;

  scanBtn.addEventListener("click", async () => {
    profileStatus.textContent = "Scanne Profile...";
    profileSelect.innerHTML = `<option>Bitte Profil wählen…</option>`;

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

  // Funktion um Geldwert zu laden
  async function updateMoneyDisplay() {
    try {
      const money = await invoke("read_money");
      moneyDisplay.textContent = `Geld: ${money.toLocaleString()} €`; // formatiert mit Tausendertrennzeichen
    } catch (error) {
      moneyDisplay.textContent = `Fehler beim Laden: ${error}`;
    }
  }

  // Beispiel: Sobald Profil geladen wird
  document
    .querySelector("#load-profile-btn")
    .addEventListener("click", async () => {
      // hier solltest du vorher load_profile aufrufen, wie in deinem bisherigen Code
      // dann Geld aktualisieren
      await updateMoneyDisplay();
    });

  profileSelect.addEventListener("change", () => {
    selectedProfilePath = profileSelect.value;
  });

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
  });

  moneyBtn.addEventListener("click", async () => {
    const amount = Number(document.querySelector("#money-input").value);
    editStatus.textContent = "Speichere...";

    await invoke("edit_money", { amount });

    editStatus.textContent = "Geld geändert!";
  });

  levelBtn.addEventListener("click", async () => {
    const xp = Number(document.querySelector("#level-input").value);
    editStatus.textContent = "Speichere...";

    await invoke("edit_level", { xp });

    editStatus.textContent = "Level geändert!";
  });
});
