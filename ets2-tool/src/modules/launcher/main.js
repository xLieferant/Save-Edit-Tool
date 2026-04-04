import { attachI18nToWindow, translateDocument } from "../shared/i18n.js";
import { safeInvoke } from "../shared/runtime.js";

const SAVE_EDITOR_PATH = "/pages/save-editor/index.html";
const CAREER_PATH = "/pages/career/index.html";

function navigateTo(path) {
  window.location.href = path;
}

document.addEventListener("DOMContentLoaded", async () => {
  attachI18nToWindow();
  await translateDocument(document);

  const saveEditorButton = document.getElementById("openSaveEditorBtn");
  const careerButton = document.getElementById("openCareerBtn");

  saveEditorButton?.addEventListener("click", async () => {
    await safeInvoke("hub_set_mode", { mode: "editor" }, { silent: true });
    navigateTo(SAVE_EDITOR_PATH);
  });

  careerButton?.addEventListener("click", async () => {
    await safeInvoke("hub_set_mode", { mode: "career" }, { silent: true });
    navigateTo(CAREER_PATH);
  });
});
