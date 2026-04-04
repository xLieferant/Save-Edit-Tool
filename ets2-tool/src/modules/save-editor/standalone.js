window.__ETS2_STANDALONE_EDITOR__ = true;

const tauriInvoke = window.__TAURI__?.core?.invoke;

async function setHubMode(mode) {
  if (!tauriInvoke) return;
  try {
    await tauriInvoke("hub_set_mode", { mode });
  } catch (error) {
    console.warn("[save-standalone] hub_set_mode failed", error);
  }
}

function interceptNavigation(element, targetPath, mode) {
  if (!element) return;

  element.addEventListener(
    "click",
    async (event) => {
      event.preventDefault();
      event.stopImmediatePropagation();
      if (mode) {
        await setHubMode(mode);
      }
      window.location.href = targetPath;
    },
    true
  );
}

document.addEventListener("DOMContentLoaded", () => {
  interceptNavigation(document.getElementById("hubHomeBtn"), "/index.html");
  interceptNavigation(document.getElementById("careerModeBtn"), "/pages/career/index.html", "career");
  interceptNavigation(document.getElementById("hubCareerCard"), "/pages/career/index.html", "career");
  interceptNavigation(document.getElementById("hubEditorCard"), "/pages/save-editor/index.html", "editor");
});
