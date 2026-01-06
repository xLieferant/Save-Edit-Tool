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
            showToast("toasts.fuel_level_updated", "success");
          }
        } catch (err) {
          console.error("Fuel level error:", err);
          showToast("Failed to change fuel level!", "error");
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
          const shouldRefuel = await openModalSlider("Refuel the truck completely?", 0);
          if (shouldRefuel) {
            await invoke("refuel_player_truck");
            await loadAllTrucks();
            showToast("Truck successfully refueled to 100%!", "success");
          }
        } catch (err) {
          console.error("Refuel error:", err);
          showToast("Failed to refuel truck!", "error");
        }
      },
      disabled: false,
    },
    {
      title: "tools.truck.mileage.title",
      desc: "tools.truck.mileage.desc",
      img: "images/odometer.png",
      action: async () => {
        try {
          const newValue = await openModalNumber(
            "Change your odometer",
            window.playerTruck?.odometer || 0
          );
          if (newValue !== null) {
            await invoke("edit_truck_odometer", { value: newValue });
            await loadAllTrucks();
            showToast(`Odometer set to ${newValue.toLocaleString()} km!`, "success");
          }
        } catch (err) {
          console.error("Odometer error:", err);
          showToast("Failed to change odometer!", "error");
        }
      },
    },
    {
      title: "tools.truck.license_plate.title",
      desc: "tools.truck.license_plate.desc",
      img: "images/trailer_license.jpg",
      action: async () => {
        try {
          const newValue = await openModalText(
            "Change your license plate",
            window.extractPlateText(window.playerTruck?.license_plate)
          );
          if (newValue !== null) {
            await invoke("set_player_truck_license_plate", { plate: newValue });
            await loadAllTrucks();
            showToast(`License plate changed to "${newValue}"!`, "success");
          }
        } catch (err) {
          console.error("License plate error:", err);
          showToast("Failed to change license plate!", "error");
        }
      },
    },
  ],

  trailer: [
    {
      title: "tools.trailer.repair.title",
      desc: "tools.trailer.repair.desc",
      img: "images/trailerRepair.jpg",
      action: trailerActionGuard(async () => {
        try {
          const shouldRepair = await openModalSlider("Repair all trailer damage?", 0);
          if (shouldRepair) {
            await invoke("repair_player_trailer");
            await loadAllTrailers();
            showToast("Trailer successfully repaired!", "success");
          }
        } catch (err) {
          console.error("Repair trailer error:", err);
          showToast("Failed to repair trailer!", "error");
        }
      }),
      disabled: false,
    },
    {
      title: "tools.trailer.license_plate.title",
      desc: "tools.trailer.license_plate.desc",
      img: "images/trailer_license.jpg",
      action: trailerActionGuard(async () => {
        try {
          const newValue = await openModalText(
            "Change trailer license",
            window.extractPlateText(window.playerTrailer?.license_plate)
          );
          if (newValue !== null) {
            await invoke("set_player_trailer_license_plate", { plate: newValue });
            await loadAllTrailers();
            showToast(`Trailer license plate changed to "${newValue}"!`, "success");
          }
        } catch (err) {
          console.error("Trailer license plate error:", err);
          showToast("Failed to change trailer license plate!", "error");
        }
      }),
      disabled: false,
    },
    {
      title: "tools.trailer.job_weight.title",
      desc: "tools.trailer.job_weight.desc",
      img: "images/comingsoon.png",
      action: trailerActionGuard(async () => {
        try {
          const newValue = await openModalNumber(
            "Modify job weight (kg)",
            window.playerTrailer?.cargo_mass || 0
          );
          if (newValue !== null) {
            await invoke("set_player_trailer_cargo_mass", { mass: newValue });
            await loadAllTrailers();
            showToast(`Cargo mass set to ${newValue.toLocaleString()} kg!`, "success");
          }
        } catch (err) {
          console.error("Cargo mass error:", err);
          showToast("Failed to change cargo mass!", "error");
        }
      }),
      disabled: false,
    },
  ],

  profile: [
    {
      title: "tools.profile.xp.title",
      desc: "tools.profile.xp.desc",
      img: "images/xp.jpg",
      action: async () => {
        try {
          const newValue = await openModalNumber(
            "Change experience",
            window.currentProfileData?.xp || 0
          );

          if (newValue !== null) {
            await invoke("edit_player_experience", { value: newValue });

            window.currentProfileData.xp = newValue;

            const xpDisplay = document.querySelector("#xpShow");
            if (xpDisplay) {
              xpDisplay.textContent = `XP: ${newValue.toLocaleString()}`;
            }

            showToast(`XP set to ${newValue.toLocaleString()}!`, "success");
          }
        } catch (err) {
          console.error("XP change error:", err);
          showToast("Failed to change XP!", "error");
        }
      },
    },
    {
      title: "tools.profile.money.title",
      desc: "tools.proifile.money.desc",
      img: "images/money.jpg",
      action: async () => {
        try {
          const newValue = await openModalNumber(
            "Change money",
            window.currentProfileData?.money || 0
          );

          if (newValue !== null) {
            await invoke("edit_player_money", { value: newValue });

            window.currentProfileData.money = newValue;

            const moneyDisplay = document.querySelector("#moneyShow");
            if (moneyDisplay) {
              moneyDisplay.textContent = `Geld: ${newValue.toLocaleString()} €`;
            }

            showToast(`Money set to ${newValue.toLocaleString()} €!`, "success");
          }
        } catch (err) {
          console.error("Money change error:", err);
          showToast("Failed to change money!", "error");
        }
      },
    },
    {
      title: "tools.profile.change_skill_points.title",
      desc: "tools.profile.change_skill_points.desc",
      img: "images/skillPoint.jpg",
      action: async () => {
        try {
          const res = await openModalMulti("Set Experience Skills", [
            {
              type: "adr",
              id: "skill_adr",
              label: "ADR",
              value: window.currentQuicksaveData?.adr || 0,
            },
            {
              type: "slider",
              id: "skill_long",
              label: "Long Distance",
              value: window.currentQuicksaveData?.long_dist || 0,
            },
            {
              type: "slider",
              id: "skill_heavy",
              label: "High Value Cargo",
              value: window.currentQuicksaveData?.heavy || 0,
            },
            {
              type: "slider",
              id: "skill_fragile",
              label: "Fragile Cargo",
              value: window.currentQuicksaveData?.fragile || 0,
            },
            {
              type: "slider",
              id: "skill_urgent",
              label: "Just in Time Delivery",
              value: window.currentQuicksaveData?.urgent || 0,
            },
            {
              type: "slider",
              id: "skill_eco",
              label: "Eco Driving",
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
            showToast("Skills successfully updated!", "success");
          }
        } catch (err) {
          console.error("Skills update error:", err);
          showToast("Failed to update skills!", "error");
        }
      },
    },
    {
      title: "tools.profile.profile_stats.title",
      desc: "tools.profile.profile_stats.desc",
      img: "images/skillPoint.jpg",
      action: async () => {
        try {
          const res = await openModalMulti("Show different stats!", [
            {
              type: "number",
              id: "stat_recruitments",
              label: "Recruitment Centers",
              value: window.currentProfileData?.recruitments || 0,
            },
            {
              type: "number",
              id: "stat_dealers",
              label: "Dealers",
              value: window.currentProfileData?.dealers || 0,
            },
            {
              type: "number",
              id: "stat_visited_cities",
              label: "Visited cities",
              value: window.currentProfileData?.visited_cities || 0,
            },
          ]);

          if (res) {
            for (const key in res) {
              await window.applySetting(key, res[key]);
            }
            showToast("Stats successfully updated!", "success");
          }
        } catch (err) {
          console.error("Stats update error:", err);
          showToast("Failed to update stats!", "error");
        }
      },
    },
    {
      title: "tools.profile.move_mods.title",
      desc: "tools.profile.move_mods.desc",
      img: "images/moveMods.png",
      action: async () => {
        try {
          if (window.handleMoveMods) {
            await window.handleMoveMods();
          }
        } catch (err) {
          console.error("Move mods error:", err);
          showToast("Failed to move modifications!", "error");
        }
      },
      disabled: false,
    }
  ],

  settings: [
    {
      title: "tools.settings.color_theme.title",
      desc: "tools.settings.color_theme.desc",
      img: "images/themeChooser.png",
      action: async () => {
        try {
          const currentTheme = localStorage.getItem("theme") || "dark";

          const res = await openModalMulti("Choose Color Theme", [
            {
              type: "dropdown",
              id: "theme",
              label: "Theme",
              value: currentTheme,
              options: ["dark", "light", "neon"],
            },
          ]);

          if (!res) return;

          const newTheme = res.theme;

          document.body.classList.remove("theme-dark", "theme-light", "theme-neon");
          document.body.classList.add(`theme-${newTheme}`);
          localStorage.setItem("theme", newTheme);

          showToast(`Theme changed to ${newTheme}!`, "success");
        } catch (err) {
          console.error("Theme change error:", err);
          showToast("Failed to change theme!", "error");
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

          const res = await openModalSlider("Enable 128 Convoy?", isActive);

          if (res !== null) {
            const value = res === 1 ? 128 : 8;
            await invoke("edit_convoy_value", { value });
            await loadBaseConfig();
            showToast(`Convoy size set to ${value}!`, "success");
          }
        } catch (err) {
          console.error("Convoy change error:", err);
          showToast("Failed to change convoy size!", "error");
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

          const res = await openModalMulti("Change Language", [
            {
              type: "dropdown",
              id: "language",
              label: "Language",
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
          showToast("Failed to change language!", "error");
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

          const newValue = await openModalNumber("g_traffic (0–10)", currentTraffic);

          if (newValue === null) return;

          const numericValue = Number(newValue);
          if (Number.isNaN(numericValue)) {
            showToast("Invalid value!", "warning");
            return;
          }

          const clamped = Math.min(10, Math.max(0, numericValue));

          await invoke("edit_traffic_value", { value: clamped });
          window.baseConfig.traffic = clamped;
          showToast(`Traffic value set to ${clamped}!`, "success");
        } catch (err) {
          console.error("Traffic Modal Error:", err);
          showToast("Failed to change traffic value!", "error");
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
            "Do you want to park doubles?",
            window.readSaveGameConfig?.factor_parking_doubles || 0
          );
          if (newValue !== null) {
            await invoke("edit_parking_doubles_value", { value: newValue });
            await loadProfileSaveConfig();
            showToast(`Parking doubles ${newValue ? 'enabled' : 'disabled'}!`, "success");
          }
        } catch (err) {
          console.error("Parking doubles error:", err);
          showToast("Failed to change parking doubles setting!", "error");
        }
      },
    },
    {
      title: "tools.settings.dev_tools.title",
      desc: "tools.settings.dev_tools.desc",
      img: "images/dev.jpg",
      action: async () => {
        try {
          const res = await openModalMulti("Developer Settings", [
            {
              type: "checkbox",
              id: "developer",
              label: "Developer",
              value: window.baseConfig?.developer,
            },
            {
              type: "checkbox",
              id: "console",
              label: "Console",
              value: window.baseConfig?.console,
            },
          ]);

          if (res) {
            await invoke("edit_developer_value", { value: res.developer });
            await invoke("edit_console_value", { value: res.console });
            await loadBaseConfig();
            showToast("Developer settings successfully updated!", "success");
          }
        } catch (err) {
          console.error("Dev mode error:", err);
          showToast("Failed to update developer settings!", "error");
        }
      },
    },
  ],
};