// Blocking head bootstrap: apply persisted appearance before styles can paint.
// No Tauri commands or savegame data are involved in appearance preferences.
(() => {
  const legacyThemes = ["theme-dark", "theme-light", "theme-neon", "theme-neon-red"];
  let current = { theme: "dark", accent: "default" };

  const normalize = (theme, accent) => ({
    theme: theme === "light" ? "light" : "dark",
    accent: accent === "green" || (theme === "neon" && accent == null) ? "green" : "default",
  });

  function render() {
    const root = document.documentElement;
    root.dataset.theme = current.theme;
    root.dataset.accent = current.accent;
    if (document.body) {
      document.body.classList.remove(...legacyThemes);
      document.body.classList.add(`theme-${current.theme}`);
      document.body.dataset.accent = current.accent;
    }
  }

  function apply(preference, { persist = true } = {}) {
    const next = normalize(preference.theme, preference.accent);
    if (persist) {
      const previousTheme = localStorage.getItem("theme");
      const previousAccent = localStorage.getItem("accent");
      try {
        localStorage.setItem("theme", next.theme);
        localStorage.setItem("accent", next.accent);
      } catch (error) {
        // Do not show an applied preference when its persistence failed.
        try {
          if (previousTheme === null) localStorage.removeItem("theme");
          else localStorage.setItem("theme", previousTheme);
          if (previousAccent === null) localStorage.removeItem("accent");
          else localStorage.setItem("accent", previousAccent);
        } catch (rollbackError) {
          console.error("[theme] preference rollback failed", rollbackError);
        }
        throw error;
      }
    }
    current = next;
    render();
    window.dispatchEvent(new CustomEvent("appearance-changed", { detail: { ...current } }));
    return { ...current };
  }

  try {
    const savedTheme = localStorage.getItem("theme");
    const savedAccent = localStorage.getItem("accent");
    current = normalize(savedTheme, savedAccent);
    if (savedTheme !== current.theme || savedAccent !== current.accent) {
      apply(current);
    }
  } catch (error) {
    console.error("[theme] preferences unavailable", error);
  }
  render();
  document.addEventListener("DOMContentLoaded", render, { once: true });
  window.addEventListener("storage", (event) => {
    if (event.key === "theme" || event.key === "accent" || event.key === null) {
      try {
        apply(normalize(localStorage.getItem("theme"), localStorage.getItem("accent")), { persist: false });
      } catch (error) {
        console.error("[theme] preference synchronization failed", error);
      }
    }
  });
  window.themePreferences = { apply, get: () => ({ ...current }) };
})();
