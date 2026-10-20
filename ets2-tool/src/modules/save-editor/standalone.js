import { selectWorkspace } from "../shared/workspace.js";

window.__ETS2_STANDALONE_EDITOR__ = true;

const GITHUB_PUBLIC_LOCK = true; // TEMP_GITHUB_BUILD

async function setHubMode(mode) {
  await selectWorkspace(mode);
}

function interceptNavigation(element, targetPath, mode, options = {}) {
  if (!element) return;

  element.addEventListener(
    "click",
    async (event) => {
      if (options.blocked) {
        event.preventDefault();
        event.stopImmediatePropagation();
        return;
      }
      event.preventDefault();
      event.stopImmediatePropagation();
      try {
        if (mode) await setHubMode(mode);
        window.location.href = targetPath;
      } catch (error) {
        console.error("[save-standalone] workspace navigation failed", error);
        window.showToast?.("ui_refinement.workspace_failed", "error");
      }
    },
    true
  );
}

document.addEventListener("DOMContentLoaded", () => {
  void selectWorkspace("editor").catch((error) => {
    console.error("[save-standalone] workspace persistence failed", error);
    window.showToast?.("ui_refinement.workspace_failed", "error");
  });
  interceptNavigation(document.getElementById("hubHomeBtn"), "/index.html?choose=1");
  interceptNavigation(
    document.getElementById("careerModeBtn"),
    "/pages/career/index.html",
    "career",
    { blocked: GITHUB_PUBLIC_LOCK }
  );
  interceptNavigation(
    document.getElementById("hubCareerCard"),
    "/pages/career/index.html",
    "career",
    { blocked: GITHUB_PUBLIC_LOCK }
  );
  interceptNavigation(document.getElementById("hubEditorCard"), "/pages/save-editor/index.html", "editor");
});
