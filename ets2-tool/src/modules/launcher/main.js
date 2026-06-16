import { attachI18nToWindow, translateDocument } from "../shared/i18n.js";
import { safeInvoke } from "../shared/runtime.js";

const SAVE_EDITOR_PATH = "/pages/save-editor/index.html";
const CAREER_PATH = "/pages/career/index.html";
const GITHUB_PUBLIC_LOCK = true; // TEMP_GITHUB_BUILD
const UPDATE_START_DELAY_MS = 2500;

function toggleGithubLockState(element, locked) {
  if (!element) return;
  element.classList.toggle("github-lock-disabled", locked);
  if (locked) {
    element.setAttribute("aria-disabled", "true");
    if ("disabled" in element) element.disabled = true;
    element.setAttribute("tabindex", "-1");
    return;
  }
  element.removeAttribute("aria-disabled");
  if ("disabled" in element) element.disabled = false;
  element.removeAttribute("tabindex");
}

function navigateTo(path) {
  window.location.href = path;
}

document.addEventListener("DOMContentLoaded", async () => {
  attachI18nToWindow();
  await translateDocument(document);
  setTimeout(async () => {
    try {
      const { checkUpdaterOnStartup } = await import("../../js/updater.js");
      await checkUpdaterOnStartup();
    } catch (error) {
      console.warn("[launcher] updater module load failed", error);
    }
  }, UPDATE_START_DELAY_MS);

  const saveEditorButton = document.getElementById("openSaveEditorBtn");
  const careerButton = document.getElementById("openCareerBtn");
  const careerCard = document.querySelector(".launcher-card-career");

  if (GITHUB_PUBLIC_LOCK) {
    document.body.classList.add("github-public-lock"); // TEMP_GITHUB_BUILD
    toggleGithubLockState(careerButton, true);
    toggleGithubLockState(careerCard, true);
  }

  saveEditorButton?.addEventListener("click", async () => {
    await safeInvoke("hub_set_mode", { mode: "editor" }, { silent: true });
    navigateTo(SAVE_EDITOR_PATH);
  });

  careerButton?.addEventListener("click", async () => {
    if (GITHUB_PUBLIC_LOCK) return; // TEMP_GITHUB_BUILD
    await safeInvoke("hub_set_mode", { mode: "career" }, { silent: true });
    navigateTo(CAREER_PATH);
  });
});
