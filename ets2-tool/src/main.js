const { invoke } = window.__TAURI__.core;

let greetInputEl;
let greetMsgEl;
let profileStatus;
let editStatus;

async function greet() {
  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  greetMsgEl.textContent = await invoke("greet", { name: greetInputEl.value });
}

window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  profileStatus = document.querySelector("#profile-status");
  editStatus = document.querySelector("#edit-status");

  document.querySelector("#greet-form").addEventListener("submit", (e) => {
    e.preventDefault();
    greet();

  document.querySelector("#load-profile-btn").addEventListener("click", loadProfile);
  document.querySelector("#save-money-btn").addEventListener("click", saveMoney);
  document.querySelector("#save-level-btn").addEventListener("click", saveLevel);
  });
});

// Lade Profil
async function loadProfile() {
  profileStatus.textContent = "Profil wird geladen…";

  try {
    const path = await invoke("load_profile");
    profileStatus.textContent = `Profil geladen: ${path}`;
  } catch (err) {
    profileStatus.textContent = "Fehler beim Laden des Profils.";
  }
}

// Geld speichern
async function saveMoney() {
  const money = document.querySelector("#money-input").value;

  if (!money) return;
  editStatus.textContent = "Wird gespeichert…";

  try {
    await invoke("edit_money", { amount: Number(money) });
    editStatus.textContent = "Geld gespeichert!";
  } catch {
    editStatus.textContent = "Fehler beim Speichern.";
  }
}

// Level speichern
async function saveLevel() {
  const level = document.querySelector("#level-input").value;

  if (!level) return;
  editStatus.textContent = "Wird gespeichert…";

  try {
    await invoke("edit_level", { level: Number(level) });
    editStatus.textContent = "Level gespeichert!";
  } catch {
    editStatus.textContent = "Fehler beim Speichern.";
  }
}