const { invoke } = window.__TAURI__.core;

document.addEventListener("DOMContentLoaded", () => {
  /* --- FOOTER/PROFILE BAR KONSTANTEN (Die neuen, stilvollen Selektoren) --- */
  const scanBtn = document.querySelector("#refreshBtn");
  // Update: Wir verwenden jetzt #profileNameDisplay (span) und #profileDropdownList (div)
  const profileNameDisplay = document.querySelector("#profileNameDisplay");
  const profileDropdownList = document.querySelector("#profileDropdownList");
  const openProfileModalBtn = document.querySelector("#openProfileModal"); // Der Pfeil-Button zum Öffnen
  const profileStatus = document.querySelector("#profile-status");

  /* --- HAUPTINHALT/DISPLAY KONSTANTEN (Aus deinem Originalcode) --- */
  // Diese Elemente müssen in deinem HTML existieren (z.B. in deinen Modals oder im Main-Bereich)
  const moneyDisplay = document.querySelector("#moneyShow");
  const xpDisplay = document.querySelector("#xpShow");

  const moneyBtn = document.querySelector("#save-money-btn");
  const levelBtn = document.querySelector("#save-level-btn");
  const editStatus = document.querySelector("#edit-status");

  // Der loadProfileBtn wurde im neuen HTML entfernt, die Logik ist jetzt im Klick-Event
  // const loadProfileBtn = document.querySelector("#load-profile-btn");

  let selectedProfilePath = null;

  // --- HILFSFUNKTIONEN FÜR CUSTOM DROPDOWN ANZEIGE ---
  function toggleProfileDropdown() {
    profileDropdownList.classList.toggle("show");
  }

  // Schließt das Dropdown, wenn man irgendwo anders klickt
  document.addEventListener("click", (event) => {
    if (!event.target.closest(".profile-picker")) {
      profileDropdownList.classList.remove("show");
    }
  });

  // Öffnet/schließt das Dropdown über den Pfeil-Button
  openProfileModalBtn.addEventListener("click", (e) => {
    e.stopPropagation(); // Verhindert, dass das document-click-Event sofort wieder schließt
    toggleProfileDropdown();
  });

  // --- PROFILE SCANNEN (Jetzt mit DIVs statt SELECT/OPTION) ---
  scanBtn.addEventListener("click", async () => {
    profileStatus.textContent = "Scanne Profile...";
    profileDropdownList.innerHTML = ""; // Liste leeren

    const profiles = await invoke("find_ets2_profiles");

    profiles.forEach((p) => {
      if (!p.success) return;

      // Erstellt ein DIV für jeden Profileintrag, um es im CSS stylen zu können
      const profileItem = document.createElement("div");
      profileItem.classList.add("dropdown-item");
      profileItem.textContent = `${p.name} (${p.path})`;
      profileItem.dataset.path = p.path; // Pfad im data-Attribut speichern

      // Event Listener für jedes Element im Dropdown
      profileItem.addEventListener("click", () => {
        selectedProfilePath = p.path;
        profileNameDisplay.textContent = p.name; // Zeigt den Namen im Haupt-Span an
        profileDropdownList.classList.remove("show"); // Dropdown schließen nach Auswahl

        // NEU: Profil direkt laden, wenn es im Custom Dropdown ausgewählt wird
        loadSelectedProfile();
      });

      profileDropdownList.appendChild(profileItem);
    });

    profileStatus.textContent = `${profiles.length} Profile gefunden`;
  });

  // --- PROFIL LADEN ---
  // Diese Funktion wird jetzt aufgerufen, wenn ein Element im Custom Dropdown geklickt wird.
  async function loadSelectedProfile() {
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
    // await updateMoneyDisplay();
    // await updateXpDisplay();
    await updateAllDisplays();
  }

  // --- ALLE DATEN AUF EINMAL LESEN ---
  async function updateAllDisplays() {
    try {
      const data = await invoke("read_all_save_data");
      window.currentProfileData = data;

      moneyDisplay.textContent = `Geld: ${data.money.toLocaleString()} €`;
      xpDisplay.textContent = `Erfahrungspunkte: ${data.xp.toLocaleString()} XP`;
      // und so weiter für die anderen Werte

      loadTools(activeTab);

    } catch (error) {
      moneyDisplay.textContent = `Fehler beim Laden: ${error}`;
      xpDisplay.textContent = `Fehler beim Laden: ${error}`;
    }
  }

  // --- GELD SPEICHERN ---
  moneyBtn.addEventListener("click", async () => {
    // Nimmt den Wert aus einem Input-Feld mit der ID 'money-input'
    const amount = Number(document.querySelector("#money-input").value);
    editStatus.textContent = "Speichere...";

    await invoke("edit_money", { amount });

    editStatus.textContent = "Geld geändert!";
    await updateMoneyDisplay();
  });

  // --- LEVEL (XP) SPEICHERN ---
  levelBtn.addEventListener("click", async () => {
    // Nimmt den Wert aus einem Input-Feld mit der ID 'level-input'
    const xp = Number(document.querySelector("#level-input").value);
    editStatus.textContent = "Speichere...";

    await invoke("edit_level", { xp });

    editStatus.textContent = "Level geändert!";
    await updateXpDisplay();
  });
});
