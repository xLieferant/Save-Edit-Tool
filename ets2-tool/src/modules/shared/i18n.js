import { hasTauri, invoke } from "./runtime.js";

export async function t(key, params = {}) {
  let text = String(key || "");
  if (hasTauri) {
    try {
      text = await invoke("translate_command", { key });
    } catch (error) {
      console.error("[i18n] translate failed", key, error);
    }
  }

  for (const [paramKey, value] of Object.entries(params)) {
    text = text.replaceAll(`{${paramKey}}`, String(value));
  }
  return text;
}

// Backend errors retain their technical details in the log. UI callers supply
// a translated, contextual fallback when the backend has no recognized key.
export async function localizeError(error, fallbackKey = "ui.operation_failed") {
  console.error("[i18n] operation error", error);
  const key = String(error?.code || error?.message || error || "");
  if (/^[a-z_]+(?:\.[a-z_0-9]+)+$/.test(key)) {
    const translated = await t(key);
    if (translated !== key) return translated;
  }
  return t(fallbackKey);
}

export async function translateDocument(root = document) {
  console.log("[i18n] loading language for document");
  if (hasTauri) {
    try {
      document.documentElement.lang = await invoke("get_current_language_command");
    } catch (error) {
      console.error("[i18n] language lookup failed", error);
    }
  }
  const translatableNodes = root.querySelectorAll("[data-translate]");
  for (const node of translatableNodes) {
    const key = node.getAttribute("data-translate");
    const translated = await t(key);
    if (translated === key) {
      console.warn("[i18n] missing translation", key);
    }
    node.textContent = translated;
  }

  const placeholderNodes = root.querySelectorAll("[data-translate-placeholder]");
  for (const node of placeholderNodes) {
    const key = node.getAttribute("data-translate-placeholder");
    const translated = await t(key);
    if (translated === key) {
      console.warn("[i18n] missing placeholder translation", key);
    }
    node.setAttribute("placeholder", translated);
  }
  for (const attribute of ["title", "aria-label"]) {
    for (const node of root.querySelectorAll(`[data-translate-${attribute}]`)) {
      const key = node.getAttribute(`data-translate-${attribute}`);
      node.setAttribute(attribute, await t(key));
    }
  }
  console.log("[i18n] loaded language for document");
}

export function attachI18nToWindow() {
  window.t = t;
  window.translateUI = () => translateDocument(document);
}
