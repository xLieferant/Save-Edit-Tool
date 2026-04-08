const tauri = window.__TAURI__;

export const hasTauri = Boolean(tauri?.core?.invoke);
export const invoke = hasTauri
  ? tauri.core.invoke
  : async () => {
    throw new Error("Tauri API not available");
  };
export const openUrl = hasTauri
  ? tauri.opener.openUrl
  : async (url) => window.open(url, "_blank", "noopener,noreferrer");
export const listen = hasTauri
  ? tauri.event.listen
  : async () => () => {};
export const convertFileSrc = hasTauri
  ? tauri.core.convertFileSrc
  : (path) => path;

export async function safeInvoke(command, args = {}, options = {}) {
  const { fallback = null, silent = false } = options;
  try {
    return await invoke(command, args);
  } catch (error) {
    if (!silent) {
      console.error(`[invoke:${command}]`, error);
    }
    return fallback;
  }
}
