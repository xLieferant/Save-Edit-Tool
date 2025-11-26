const { invoke } = window.__TAURI__.core;

document.addEventListener("DOMContentLoaded", () => {
  const scanBtn = document.querySelector("#scan-profiles-btn");
  const profileSelect = document.querySelector("#profile-select");
  const loadProfileBtn = document.querySelector("#load-profile-btn");
  const profileStatus = document.querySelector("#profile-status");

  const moneyBtn = document.querySelector("#save-money-btn");
  const levelBtn = document.querySelector("#save-level-btn");
  const editStatus = document.querySelector("#edit-status");

  let selectedProfilePath = null;

  // Profile scannen
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

  // Profil auswählen
  profileSelect.addEventListener("change", () => {
    selectedProfilePath = profileSelect.value;
  });

  // Profil laden → autosave öffnen
  loadProfileBtn.addEventListener("click", async () => {
    if (!selectedProfilePath) {
      profileStatus.textContent = "Kein Profil ausgewählt!";
      return;
    }

    profileStatus.textContent = "Lade autosave/info.sii...";

    const result = await invoke("load_profile", { path: selectedProfilePath });

    profileStatus.textContent = result;
  });

  // Geld speichern
  moneyBtn.addEventListener("click", async () => {
    const amount = Number(document.querySelector("#money-input").value);
    editStatus.textContent = "Speichere...";

    await invoke("edit_money", { amount });

    editStatus.textContent = "Geld geändert!";
  });

  // Level speichern
  levelBtn.addEventListener("click", async () => {
    const level = Number(document.querySelector("#level-input").value);
    editStatus.textContent = "Speichere...";

    await invoke("edit_level", { level });

    editStatus.textContent = "Level geändert!";
  });
});
