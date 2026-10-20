import { attachI18nToWindow, translateDocument } from "../shared/i18n.js";
import { savedWorkspace, selectWorkspace, workspacePath } from "../shared/workspace.js";

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
  const shell = document.querySelector(".launcher-shell");
  if (!new URL(window.location.href).searchParams.has("choose")) {
    try {
      const saved = await savedWorkspace(GITHUB_PUBLIC_LOCK ? ["editor"] : ["editor", "career"]);
      if (saved) {
        window.location.replace(workspacePath(saved));
        return;
      }
    } catch (error) {
      console.error("[launcher] workspace lookup failed", error);
    }
  }
  if (shell) shell.hidden = false;
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

  const openWorkspace = async (mode, button) => {
    button.disabled = true;
    button.setAttribute("aria-busy", "true");
    try {
      navigateTo(await selectWorkspace(mode));
    } catch (error) {
      console.error("[launcher] workspace selection failed", error);
      const message = document.getElementById("launcherWorkspaceError");
      if (message) message.hidden = false;
      button.disabled = false;
      button.removeAttribute("aria-busy");
    }
  };

  saveEditorButton?.addEventListener("click", () => openWorkspace("editor", saveEditorButton));

  careerButton?.addEventListener("click", async () => {
    if (GITHUB_PUBLIC_LOCK) return; // TEMP_GITHUB_BUILD
    await openWorkspace("career", careerButton);
  });
});
