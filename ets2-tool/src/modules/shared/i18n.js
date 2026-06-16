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

export async function translateDocument(root = document) {
  console.log("[i18n] loading language for document");
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
  console.log("[i18n] loaded language for document");
}

export function attachI18nToWindow() {
  window.t = t;
  window.translateUI = () => translateDocument(document);
}
