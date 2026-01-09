import {
  openModalNumber,
  openModalText,
  openModalSlider,
  openModalMulti,
  openCloneProfileModal,
} from "./app.js";

// Helper function to guard trailer actions
const trailerActionGuard = (actionFunction) => async (...args) => {
  if (!window.playerTrailer) {
    showToast("toasts.no_trailer_assigned_error", "warning");
    return;
  }
  await actionFunction(...args);
};

const GAME_IMAGE_CATEGORIES = ["truck", "trailer", "profile"];
const BASE_IMAGE_PREFIX = "images/";
const ATS_IMAGE_PREFIX = "images/ATS/";

function resolveGameToolImage(baseImg, game) {
  if (game !== "ats") return baseImg;
  if (!baseImg || typeof baseImg !== "string") return baseImg;
  if (baseImg.startsWith(ATS_IMAGE_PREFIX)) return baseImg;
  if (baseImg.startsWith(BASE_IMAGE_PREFIX)) {
    return `${ATS_IMAGE_PREFIX}${baseImg.slice(BASE_IMAGE_PREFIX.length)}`;
  }
  return baseImg;
}

// --------------------------------------------------------------
// TOOL DEFINITIONS
// --------------------------------------------------------------
export const tools = {
  truck: [
    {
      title: "tools.truck.repair_truck.title",
      desc: "tools.truck.repair_truck.desc",
      img: "images/repair.png",
      action: async () => {
        try {
          const shouldRepair = await openModalSlider("tools.truck.repair_truck.modalSliderText", 0);
          if (shouldRepair) {
            const wearTypes = ["engine_wear", "transmission_wear", "cabin_wear", "chassis_wear"];
            for (const wearType of wearTypes) {
              await invoke("set_player_truck_wear", {
                wearType: wearType,
                level: 0.0,
              });
            }
            await loadAllTrucks();
            showToast("toasts.repair_truck_success", "success");
          }
        } catch (err) {
          console.error("errors.repair_truck", err);
          showToast("toasts.repair_truck_error", "error");
        }
      },
      disabled: false,
    },
    {
      title: "tools.truck.advanced_repair.title",
      desc: "tools.truck.advanced_repair.desc",
      img: "images/advancedRepair.png",
      action: async () => {
        try {
          const res = await openModalMulti("tools.truck.advanced_repair.modalSliderText", [
            {
              type: "slider",
              id: "engine_wear",
              label: "label.engine_wear",
              value: window.playerTruck?.engine_wear || 0,
              max: 1,
              step: 0.01,
            },
            {
              type: "slider",
              id: "transmission_wear",
              label: "label.transmission_wear",
              value: window.playerTruck?.transmission_wear || 0,
              max: 1,
              step: 0.01,
            },
            {
              type: "slider",
              id: "cabin_wear",
              label: "label.cabin_wear",
              value: window.playerTruck?.cabin_wear || 0,
              max: 1,
              step: 0.01,
            },
            {
              type: "slider",
              id: "chassis_wear",
              label: "label.chassis_wear",
              value: window.playerTruck?.chassis_wear || 0,
              max: 1,
              step: 0.01,
            },
          ]);

          if (res) {
            for (const key in res) {
              await invoke("set_player_truck_wear", {
                wearType: key,
                level: res[key],
              });
            }
            await loadAllTrucks();
            showToast("toasts.advanced_repair_success", "success");
          }
        } catch (err) {
          console.error("Advanced repair error:", err);
          showToast("toasts.advanced_repair_error", "error");
        }
      },
    },
    {
      title: "tools.truck.fuel_level.title",
      desc: "tools.truck.fuel_level.desc",
      img: "images/gasstation.jpg",
      action: async () => {
        try {
          const currentFuelPercent = (window.playerTruck?.fuel_relative || 0) * 100;
          const newValue = await openModalNumber("tools.truck.fuel_level.modalNumberText", currentFuelPercent.toFixed(0));
          if (newValue !== null) {
            const clampedValue = Math.max(0, Math.min(100, newValue));
            const finalValue = clampedValue / 100.0;
            await invoke("set_player_truck_fuel", { level: finalValue });
            await loadAllTrucks();
            // showToast(`Fuel level set to ${clampedValue}%!`, "success");
            showToast("toasts.fuel_level_updated", { clampedValue }, "success");
          }
        } catch (err) {
          console.error("errors.fuel_level", err);
          showToast("toasts.fuel_level_error", "error");
        }
      },
      disabled: false,
    },
    {
      title: "tools.truck.full_refuel.title",
      desc: "tools.truck.full_refuel.desc",
      img: "images/gasstation.jpg",
      action: async () => {
        try {
          const shouldRefuel = await openModalSlider("tools.truck.full_refuel.modalSliderText", 0);
          if (shouldRefuel) {
            await invoke("refuel_player_truck");
            await loadAllTrucks();
            showToast("toasts.fuel_refuel_success", "success");
          }
        } catch (err) {
          console.error("Refuel error:", err);
          showToast("toasts.fuel_refuel_error", "error");
        }
      },
      disabled: false,
    },
    {
      title: "tools.truck.truck_mileage.title",
      desc: "tools.truck.truck_mileage.desc",
      img: "images/odometer.png",
      action: async () => {
        try {
          const newValue = await openModalNumber(
            "tools.truck.truck_mileage.modalNumberText",
            window.playerTruck?.odometer || 0
          );
          if (newValue !== null) {
            await invoke("edit_truck_odometer", { value: newValue });
            await loadAllTrucks();
            showToast("toasts.truck_mileage_success", { newValue }, "success");
          }
        } catch (err) {
          console.error("Odometer error:", err);
          showToast("toasts.truck_mileage_error", "error");
        }
      },
    },
    {
      title: "tools.truck.truck_license_plate.title",
      desc: "tools.truck.truck_license_plate.desc",
      img: "images/trailer_license.jpg",
      action: async () => {
        try {
          const newValue = await openModalText(
            "tools.truck.truck_license_plate.modalTextTitle",
            window.extractPlateText(window.playerTruck?.license_plate)
          );
          if (newValue !== null) {
            await invoke("set_player_truck_license_plate", { plate: newValue });
            await loadAllTrucks();
            showToast("toasts.truck_license_plate_success", { newValue }, "success");
          }
        } catch (err) {
          console.error("License plate error:", err);
          showToast("toasts.truck_license_plate_error", "error");
        }
      },
    },
  ],

  trailer: [
    {
      title: "tools.trailer.repair_trailer.title",
      desc: "tools.trailer.repair_trailer.desc",
      img: "images/trailerRepair.jpg",
      action: trailerActionGuard(async () => {
        try {
          const shouldRepair = await openModalSlider("tools.trailer.repair_trailer.modalSliderText", 0);
          if (shouldRepair) {
            await invoke("repair_player_trailer");
            await loadAllTrailers();
            showToast("toasts.repair_trailer_success", "success");
          }
        } catch (err) {
          console.error("Repair trailer error:", err);
          showToast("toasts.repair_trailer_error", "error");
        }
      }),
      disabled: false,
    },
    {
      title: "tools.trailer.trailer_license_plate.title",
      desc: "tools.trailer.trailer_license_plate.desc",
      img: "images/trailer_license.jpg",
      action: trailerActionGuard(async () => {
        try {
          const newValue = await openModalText(
            "tools.trailer.trailer_license_plate.modalTextTitle",
            window.extractPlateText(window.playerTrailer?.license_plate)
          );
          if (newValue !== null) {
            await invoke("set_player_trailer_license_plate", { plate: newValue });
            await loadAllTrailers();
            showToast("toasts.trailer_license_plate_success", { newValue }, "success");
          }
        } catch (err) {
          console.error("Trailer license plate error:", err);
          showToast("toasts.trailer_license_plate_error", "error");
        }
      }),
      disabled: false,
    },
    {
      title: "tools.trailer.modify_job_weight.title",
      desc: "tools.trailer.modify_job_weight.desc",
      img: "images/comingsoon.png",
      action: trailerActionGuard(async () => {
        try {
          const newValue = await openModalNumber(
            "tools.trailer.modify_job_weight.modalNumberText",
            window.playerTrailer?.cargo_mass || 0
          );
          if (newValue !== null) {
            await invoke("set_player_trailer_cargo_mass", { mass: newValue });
            await loadAllTrailers();
            showToast("toasts.modify_job_weight_success", { newValue }, "success");
          }
        } catch (err) {
          console.error("Cargo mass error:", err);
          showToast("toasts.modify_job_weight_error", "error");
        }
      }),
      disabled: false,
    },
  ],

  profile: [
    {
      title: "tools.profile.change_xp.title",
      desc: "tools.profile.change_xp.desc",
      img: "images/xp.jpg",
      action: async () => {
        try {
          const newValue = await openModalNumber(
            "tools.profile.change_xp.modalNumberText",
            window.currentProfileData?.xp || 0
          );

          if (newValue !== null) {
            await invoke("edit_player_experience", { value: newValue });

            window.currentProfileData.xp = newValue;

            const xpDisplay = document.querySelector("#xpShow");
            if (xpDisplay) {
              xpDisplay.textContent = `XP: ${newValue.toLocaleString()}`;
            }

            showToast("toasts.change_xp_success", { newValue }, "success");
          }
        } catch (err) {
          console.error("XP change error:", err);
          showToast("toasts.change_xp_error", "error");
        }
      },
    },
    {
      title: "tools.profile.change_money.title",
      desc: "tools.profile.change_money.desc",
      img: "images/money.jpg",
      action: async () => {
        try {
          const newValue = await openModalNumber(
            "tools.profile.change_money.modalNumberText",
            window.currentProfileData?.money || 0
          );

          if (newValue !== null) {
            await invoke("edit_player_money", { value: newValue });

            window.currentProfileData.money = newValue;

            const moneyDisplay = document.querySelector("#moneyShow");
            if (moneyDisplay) {
              moneyDisplay.textContent = `Geld: ${newValue.toLocaleString()} €`;
            }

            showToast("toasts.change_money_success", { newValue }, "success");
          }
        } catch (err) {
          console.error("Money change error:", err);
          showToast("toasts.change_money_error", "error");
        }
      },
    },
    {
      title: "tools.profile.change_skill_points.title",
      desc: "tools.profile.change_skill_points.desc",
      img: "images/skillPoint.jpg",
      action: async () => {
        try {
          const res = await openModalMulti("tools.profile.change_skill_points.modalTextTitle", [
            {
              type: "adr",
              id: "skill_adr",
              label: "label.adr",
              value: window.currentQuicksaveData?.adr || 0,
            },
            {
              type: "slider",
              id: "skill_long",
              label: "label.long_distance",
              value: window.currentQuicksaveData?.long_dist || 0,
            },
            {
              type: "slider",
              id: "skill_heavy",
              label: "label.heavy_cargo",
              value: window.currentQuicksaveData?.heavy || 0,
            },
            {
              type: "slider",
              id: "skill_fragile",
              label: "label.fragile_cargo",
              value: window.currentQuicksaveData?.fragile || 0,
            },
            {
              type: "slider",
              id: "skill_urgent",
              label: "label.just_in_time_delivery",
              value: window.currentQuicksaveData?.urgent || 0,
            },
            {
              type: "slider",
              id: "skill_eco",
              label: "label.eco_driving",
              value: window.currentQuicksaveData?.mechanical || 0,
            },
          ]);

          if (res) {
            await invoke("edit_skill_value", { skill: "adr", value: res.skill_adr });
            await invoke("edit_skill_value", { skill: "long_dist", value: res.skill_long });
            await invoke("edit_skill_value", { skill: "heavy", value: res.skill_heavy });
            await invoke("edit_skill_value", { skill: "fragile", value: res.skill_fragile });
            await invoke("edit_skill_value", { skill: "urgent", value: res.skill_urgent });
            await invoke("edit_skill_value", { skill: "mechanical", value: res.skill_eco });

            await loadQuicksave();
            showToast("toasts.change_skill_points_success", "success");
          }
        } catch (err) {
          console.error("Skills update error:", err);
          showToast("toasts.change_skill_points_error", "error");
        }
      },
    },
    {
      title: "tools.profile.profile_stats.title",
      desc: "tools.profile.profile_stats.desc",
      img: "images/skillPoint.jpg",
      action: async () => {
        try {
          const res = await openModalMulti("tools.profile.profile_stats.modalTextTitle", [
            {
              type: "number",
              id: "stat_recruitments",
              label: "label.recruitment_centers",
              value: window.currentProfileData?.recruitments || 0,
            },
            {
              type: "number",
              id: "stat_dealers",
              label: "label.dealers_visited",
              value: window.currentProfileData?.dealers || 0,
            },
            {
              type: "number",
              id: "stat_visited_cities",
              label: "label.visited_cities",
              value: window.currentProfileData?.visited_cities || 0,
            },
          ]);

          if (res) {
            for (const key in res) {
              await window.applySetting(key, res[key]);
            }
            showToast("toasts.profile_stats_success", "success");
          }
        } catch (err) {
          console.error("Stats update error:", err);
          showToast("toasts.profile_stats_error", "error");
        }
      },
    },
    {
      title: "tools.profile.move_mods.title",
      desc: "tools.profile.move_mods.desc",
      img: "images/moveMods.png",
      disabled: false,

      action: async () => {
        const choice = await openModalMulti("tools.profile.move_mods.modalTextTitle", [
          {
            type: "dropdown",
            id: "action",
            label: "label.action_move_mods",
            value: "label.value_move_mods",
            options: ["label.label_move_mods", "label.label_move_controls"],
          },
        ]);

        if (!choice) return;

        switch (choice.action) {
          case "label.label_move_mods":
            if (window.handleMoveMods) {
              await window.handleMoveMods();
            }
          break;

          case "label.label_move_controls":
            if (window.handleCopyControls) {
              await window.handleCopyControls();
            }
          break;

          default:
            console.warn("Unknown action:", choice.action);
        }
      },
    },
  ],

  settings: [
    {
      title: "tools.settings.color_theme.title",
      desc: "tools.settings.color_theme.desc",
      img: "images/themeChooser.png",
      action: async () => {
        try {
          const currentTheme = localStorage.getItem("theme") || "dark";
          
          // Internal values map
          const themeMap = {
            "label.label_color_theme_dark": "dark",
            "label.label_color_theme_light": "light",
            "label.label_color_theme_neon": "neon"
          };
          
          // Reverse map to find key for current theme
          const currentKey = Object.keys(themeMap).find(key => themeMap[key] === currentTheme) || "label.label_color_theme_dark";

          const res = await openModalMulti("tools.settings.color_theme.modalTextTitle", [
            {
              type: "dropdown",
              id: "theme",
              label: "label.label_theme",
              value: currentKey,
              options: Object.keys(themeMap),
            },
          ]);

          if (!res) return;

          // Lookup internal value from selected key
          const newTheme = themeMap[res.theme];

          if (newTheme) {
            document.body.classList.remove("theme-dark", "theme-light", "theme-neon");
            document.body.classList.add(`theme-${newTheme}`);
            localStorage.setItem("theme", newTheme);
            showToast("toasts.color_theme_success", { newTheme }, "success");
          } else {
             console.error("Unknown theme selected:", res.theme);
             showToast("toasts.color_theme_error", "error");
          }
        } catch (err) {
          console.error("Theme change error:", err);
          showToast("toasts.color_theme_error", "error");
        }
      },
      disabled: false,
    },
    {
      title: "tools.settings.convoy.title",
      desc: "tools.settings.convoy.desc",
      img: "images/convoy.jpg",
      action: async () => {
        try {
          const isActive = window.baseConfig?.max_convoy_size === 128 ? 1 : 0;

          const res = await openModalSlider("tools.settings.convoy.modalTextTitle", isActive);

          if (res !== null) {
            const value = res === 1 ? 128 : 8;
            await invoke("edit_convoy_value", { value });
            await loadBaseConfig();
            showToast("toasts.convoy_settings_success", { newValue: value }, "success");
          }
        } catch (err) {
          console.error("Convoy change error:", err);
          showToast("toasts.convoy_settings_error", "error");
        }
      },
    },
    {
      title: "tools.settings.language.title",
      desc: "tools.settings.language.desc",
      img: "images/language.png",
      action: async () => {
        try {
          // Daten aus Backend holen
          const languages = await invoke("get_available_languages_command");
          const currentLang = await invoke("get_current_language_command");

          if (!languages || languages.length === 0) {
            showToast("No languages available!", "error");
            return;
          }

          // Dropdown-Optionen vorbereiten
          const options = languages.map(l => ({
            value: l.code,
            label: l.name,
          }));

          const res = await openModalMulti("tools.settings.language.modalTextTitle", [
            {
              type: "dropdown",
              id: "language",
              label: "label.label_language",
              value: currentLang,
              options: options.map(o => o.value),
              optionLabels: options.reduce((acc, o) => {
                acc[o.value] = o.label;
                return acc;
              }, {}),
            },
          ]);

          if (!res || !res.language) return;

          if (res.language === currentLang) {
            showToast("Language already active.", "info");
            return;
          }

          // Sprache setzen
          const message = await invoke("set_language_command", {
            language: res.language,
          });

          showToast(message, "success");

          // OPTIONAL (empfohlen, wenn UI statisch übersetzt ist)
          location.reload();

        } catch (err) {
          console.error("Language modal error:", err);
          showToast("toasts.language_update_error", "error");
        }
      },
    },
    {
      title: "tools.settings.traffic_values.title",
      desc: "tools.settings.traffic_values.desc",
      img: "images/traffic_value.png",
      action: async () => {
        try {
          const currentTraffic = await invoke("read_traffic_value");

          const newValue = await openModalNumber("tools.settings.traffic_values.modalTextTitle", currentTraffic);

          if (newValue === null) return;

          const numericValue = Number(newValue);
          if (Number.isNaN(numericValue)) {
            showToast("Invalid value!", "warning");
            return;
          }

          const clamped = Math.min(10, Math.max(0, numericValue));

          await invoke("edit_traffic_value", { value: clamped });
          window.baseConfig.traffic = clamped;
          showToast("toasts.traffic_values_success", { newValue: clamped }, "success");
        } catch (err) {
          console.error("Traffic Modal Error:", err);
          showToast("toasts.traffic_values_error", "error");
        }
      },
    },
    {
      title: "tools.settings.parking_doubles.title",
      desc: "tools.settings.parking_doubles.desc",
      img: "images/parking_double.png",
      action: async () => {
        try {
          const newValue = await openModalSlider(
            "tools.settings.parking_doubles.modalTextTitle",
            window.readSaveGameConfig?.factor_parking_doubles || 0
          );
          if (newValue !== null) {
            await invoke("edit_parking_doubles_value", { value: newValue });
            await loadProfileSaveConfig();
            showToast("toasts.parking_doubles_success", { newValue: newValue ? "enabled" : "disabled" }, "success");
          }
        } catch (err) {
          console.error("Parking doubles error:", err);
          showToast("toasts.parking_doubles_error", "error");
        }
      },
    },
    {
      title: "tools.settings.dev_tools.title",
      desc: "tools.settings.dev_tools.desc",
      img: "images/dev.jpg",
      action: async () => {
        try {
          const res = await openModalMulti("tools.settings.dev_tools.modalTextTitle", [
            {
              type: "checkbox",
              id: "developer",
              label: "label.label_developer",
              value: window.baseConfig?.developer,
            },
            {
              type: "checkbox",
              id: "console",
              label: "label.label_console",
              value: window.baseConfig?.console,
            },
          ]);

          if (res) {
            await invoke("edit_developer_value", { value: res.developer });
            await invoke("edit_console_value", { value: res.console });
            await loadBaseConfig();
            showToast("toasts.dev_tools_success", "success");
          }
        } catch (err) {
          console.error("Dev mode error:", err);
          showToast("toasts.dev_tools_error", "error");
        }
      },
    },
  ],
};

export function updateToolImagesForGame(game) {
  GAME_IMAGE_CATEGORIES.forEach((category) => {
    tools[category].forEach((tool) => {
      if (!tool.baseImg) {
        tool.baseImg = tool.img;
      }
      tool.img = resolveGameToolImage(tool.baseImg, game);
    });
  });
}
