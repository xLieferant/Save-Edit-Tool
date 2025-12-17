const { invoke } = window.__TAURI__.core;

window.applySetting = async function (key, value) {
  await invoke("apply_setting", {
    payload: { key, value }
  });

  // Auto Reload
  if (window.loadProfileData) await loadProfileData();
  if (window.loadQuicksave) await loadQuicksave();
  if (window.loadBaseConfig) await loadBaseConfig();
};
