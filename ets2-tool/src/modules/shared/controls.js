import { t } from "./i18n.js";

const bindings = new WeakMap();

export function booleanValue(value) {
  return value === true || value === 1 || value === "1" || value === "true";
}

export async function bindSwitch(input) {
  const label = input.closest(".toggle-switch");
  const state = label?.querySelector(".toggle-switch__state");
  input.setAttribute("role", "switch");
  const sync = async () => {
    label?.classList.toggle("toggle-switch--active", input.checked);
    input.setAttribute("aria-checked", String(input.checked));
    if (state) {
      const key = input.checked ? "label.toggle_on" : "label.toggle_off";
      state.dataset.translate = key;
      const text = await t(key);
      if (state.dataset.translate === key) state.textContent = text;
    }
  };
  input.addEventListener("change", sync);
  bindings.set(input, sync);
  await sync();
  return () => {
    input.removeEventListener("change", sync);
    bindings.delete(input);
  };
}

export function syncSwitch(input) {
  bindings.get(input)?.();
}

export async function mountSwitches(root = document) {
  for (const label of root.querySelectorAll("label.career-toggle-field, label.career-setting-row")) {
    const input = label.querySelector('input[type="checkbox"]');
    if (!input || bindings.has(input)) continue;
    label.classList.add("toggle-switch", "toggle-switch--field");
    input.classList.add("toggle-switch__input");
    input.insertAdjacentHTML("afterend", '<span class="toggle-switch__slider" aria-hidden="true"></span><span class="toggle-switch__state"></span>');
    await bindSwitch(input);
  }
}
