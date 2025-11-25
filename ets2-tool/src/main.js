const { invoke } = window.__TAURI__.core;

document.addEventListener("DOMContentLoaded", () => {
  const scanBtn = document.querySelector("#scan-profiles-btn");
  const profileStatus = document.querySelector("#profile-status");
  const profileList = document.querySelector("#profile-list");

  const moneyBtn = document.querySelector("#save-money-btn");
  const levelBtn = document.querySelector("#save-level-btn");
  const editStatus = document.querySelector("#edit-status");

  // Profile suchen
  scanBtn.addEventListener("click", async () => {
    profileStatus.textContent = "Suche nach ETS2-Profilen...";
    profileList.innerHTML = "";

    const profiles = await invoke("find_ets2_profiles");

    if (profiles.length === 0) {
      profileStatus.textContent = "Keine Profile gefunden.";
      return;
    }

    profileStatus.textContent = `Gefundene Profile: ${profiles.length}`;

   profiles.forEach((p) => {
  const li = document.createElement("li");
  if (p.success) {
    li.textContent = `Profil: ${p.name} ✅ (${p.path})`;
  } else {
    li.textContent = `Fehler bei ${p.path} ❌`;
  }
  profileList.appendChild(li);
});

  });

  // Geld speichern
  moneyBtn.addEventListener("click", async () => {
    const val = Number(document.querySelector("#money-input").value);

    editStatus.textContent = "Speichere...";

    await invoke("edit_money", { amount: val });

    editStatus.textContent = "Geld gespeichert!";
  });

  // Level speichern
  levelBtn.addEventListener("click", async () => {
    const val = Number(document.querySelector("#level-input").value);

    editStatus.textContent = "Speichere...";

    await invoke("edit_level", { level: val });

    editStatus.textContent = "Level gespeichert!";
  });
});
