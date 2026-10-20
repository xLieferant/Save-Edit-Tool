import { t } from "./i18n.js";

function updatePending(panel) {
  if (panel) panel.dataset.settingsDirty = String(Boolean(panel.querySelector('[data-dirty="true"]')));
}

export async function protectSettingsForms(root = document) {
  const message = await t("ui_refinement.unsaved_changes");
  for (const panel of root.querySelectorAll("#panel-settings, #panel-company")) {
    const hint = document.createElement("p");
    hint.className = "settings-pending";
    hint.dataset.translate = "ui_refinement.unsaved_changes";
    hint.setAttribute("role", "status");
    hint.textContent = message;
    panel.appendChild(hint);
  }
  const mark = (event) => {
    const input = event.target.closest("input, select, textarea");
    const panel = input?.closest("#panel-settings, #panel-company");
    if (!input || !panel || input.disabled) return;
    if (["cmSettingAutoRefresh", "cmSettingMockFallback"].includes(input.id)) return;
    if (!/^cm(?:User|Company|Career)/.test(input.id) || input.id === "cmUserUsernameInput") return;
    input.dataset.dirty = "true";
    updatePending(panel);
  };
  root.addEventListener("input", mark);
  root.addEventListener("change", mark);
}

export function beginSettingsSave(buttonId, fieldIds, renderConfirmed) {
  const button = document.getElementById(buttonId);
  if (!button || button.disabled) return null;
  const panel = button.closest(".career-panel");
  if (panel?.dataset.settingsSaving === "true") return null;
  if (panel) panel.dataset.settingsSaving = "true";
  const fields = fieldIds.map((id) => document.getElementById(id)).filter(Boolean);
  const controls = [...new Set([button, ...fields, ...(panel?.querySelectorAll("button") || [])])];
  const previousDisabled = controls.map((control) => control.disabled);
  const panels = new Set(fields.map((field) => field.closest(".career-panel")));
  controls.forEach((control) => { control.disabled = true; });
  button.setAttribute("aria-busy", "true");
  fields.forEach((field) => field.closest(".toggle-switch")?.setAttribute("aria-busy", "true"));
  const clearDirty = () => {
    fields.forEach((field) => { delete field.dataset.dirty; });
    panels.forEach(updatePending);
  };
  return {
    commit: clearDirty,
    rollback() {
      clearDirty();
      renderConfirmed();
    },
    finish() {
      controls.forEach((control, index) => { control.disabled = previousDisabled[index]; });
      button.removeAttribute("aria-busy");
      if (panel) delete panel.dataset.settingsSaving;
      fields.forEach((field) => field.closest(".toggle-switch")?.removeAttribute("aria-busy"));
    },
  };
}
