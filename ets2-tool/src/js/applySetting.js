import { SETTINGS_MAP } from "./settings.map.js";
const { invoke } = window.__TAURI__.core;

export async function applySetting(settingId, rawValue) {
  const cfg = SETTINGS_MAP[settingId];
  if (!cfg) throw new Error(`Unknown setting: ${settingId}`);

  let value;

  switch (cfg.type) {
    case "number":
      value = Number(rawValue);
      if (Number.isNaN(value)) throw new Error("Invalid number");
      break;

    case "bool":
      value = rawValue ? "1" : "0";
      break;

    case "adr":
      value = String(rawValue);
      break;

    case "string":
      value = String(rawValue);
      break;

    default:
      throw new Error("Unknown type");
  }

  await invoke("apply_setting", {
    key: cfg.key,
    value: String(value),
    fileType: cfg.file
  });

  await autoReload(cfg.reload);
}

async function autoReload(list = []) {
  for (const r of list) {
    if (r === "profile") {
      window.currentProfileData = await invoke("read_all_save_data");
    }
    if (r === "quicksave") {
      window.currentQuicksaveData = await invoke("quicksave_game_info");
    }
    if (r === "baseConfig") {
      window.baseConfig = await invoke("read_base_config");
    }
    if (r === "saveConfig") {
      window.readSaveGameConfig = await invoke("read_save_config", {
        profilePath: window.selectedProfilePath
      });
    }
  }

  loadTools(activeTab);
}
