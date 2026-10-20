import { hasTauri, invoke } from "./runtime.js";

const storageKey = "ets2_workspace";
const paths = { editor: "/pages/save-editor/index.html", career: "/pages/career/index.html" };
const normalize = (mode) => mode === "utility" ? "editor" : mode;

export function workspacePath(mode) {
  return paths[normalize(mode)] || null;
}

export async function selectWorkspace(mode) {
  const selected = normalize(mode);
  if (!workspacePath(selected)) throw new Error("Invalid workspace");
  if (hasTauri) await invoke("hub_set_mode", { mode: selected });
  localStorage.setItem(storageKey, selected);
  return workspacePath(selected);
}

export async function savedWorkspace(available = ["editor", "career"]) {
  const stored = normalize(localStorage.getItem(storageKey));
  if (!workspacePath(stored) || !available.includes(stored)) return null;
  if (hasTauri) {
    try {
      const backendMode = normalize(await invoke("hub_get_mode"));
      if (backendMode !== stored) return null;
    } catch (error) {
      console.error("[workspace] saved selection validation failed", error);
      return null;
    }
  }
  return stored;
}
