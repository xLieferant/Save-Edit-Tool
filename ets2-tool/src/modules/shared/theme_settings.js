import { t } from "./i18n.js";

export async function openThemeSettings(openModal) {
  try {
    const current = window.themePreferences.get();
    const result = await openModal("tools.settings.color_theme.modalTextTitle", [
      {
        type: "dropdown",
        id: "theme",
        label: "label.label_theme",
        value: current.theme,
        options: ["dark", "light"],
        optionLabels: {
          dark: await t("label.label_color_theme_dark"),
          light: await t("label.label_color_theme_light"),
        },
      },
      {
        type: "dropdown",
        id: "accent",
        label: "ui_refinement.accent_label",
        value: current.accent,
        options: ["default", "green"],
        optionLabels: {
          default: await t("ui_refinement.accent_default"),
          green: await t("ui_refinement.accent_green"),
        },
      },
    ]);
    if (!result) return;
    window.themePreferences.apply(result);
    window.showToast?.("ui_refinement.appearance_saved", "success");
  } catch (error) {
    console.error("[theme] appearance change failed", error);
    window.showToast?.("toasts.color_theme_error", "error");
  }
}
