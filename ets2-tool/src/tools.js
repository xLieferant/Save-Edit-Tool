import {
  openModalNumber,
  openModalText,
  openModalSlider,
  openModalMulti,
  openCloneProfileModal,
  openCurrentTruckModal,
  openTruckChangeModal,
  openTrailerChangeModal,
  openLevelSystemModal,
  openModConflictDiagnosticsPage,
  openModProfileManagerPage,
  openProfileSharingPage,
} from "./app.js";

const TRAILER_LICENSE_PLATE_MAX_LENGTH = 32;
const JOB_WEIGHT_MAX_KG = 1000000;

function extractTrailerPlateText(plate) {
  if (!plate) return "";
  return String(plate)
    .replace(/^"|"$/g, "")
    .split("|")[0]
    .replace(/<[^>]*>/g, " ")
    .replace(/[\u0000-\u001f\u007f]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function requireTrailerModal(selector, actionName) {
  const modalElement = document.querySelector(selector);
  console.debug(`[Trailer] ${actionName} modal lookup`, {
    selector,
    found: Boolean(modalElement),
  });
  if (!modalElement) {
    throw new Error("trailer_modal_not_found");
  }
  return modalElement;
}

async function openJobWeightModal(currentValue) {
  const modalInput = requireTrailerModal("#modalNumberInput", "job_weight");
  const originalType = modalInput.getAttribute("type");
  const originalInputMode = modalInput.getAttribute("inputmode");
  modalInput.setAttribute("type", "text");
  modalInput.setAttribute("inputmode", "decimal");

  try {
    const submittedValue = await openModalNumber(
      "tools.trailer.modify_job_weight.modalNumberText",
      currentValue
    );
    return {
      submittedValue,
      rawValue: modalInput.value.trim(),
    };
  } finally {
    if (originalType === null) {
      modalInput.removeAttribute("type");
    } else {
      modalInput.setAttribute("type", originalType);
    }
    if (originalInputMode === null) {
      modalInput.removeAttribute("inputmode");
    } else {
      modalInput.setAttribute("inputmode", originalInputMode);
    }
  }
}

function showTrailerActionError(error, fallbackKey) {
  const errorCode = String(error);
  const mappings = [
    ["no_active_job", "toasts.trailer_no_active_job"],
    ["active_job_trailer_not_found", "toasts.no_trailer_assigned_error"],
    ["active_trailer_not_found", "toasts.no_trailer_assigned_error"],
    ["active_trailer_not_editable", "toasts.no_trailer_assigned_error"],
    ["trailer_license_plate_empty", "toasts.trailer_license_plate_empty"],
    ["trailer_license_plate_invalid", "toasts.trailer_license_plate_invalid"],
    ["trailer_license_plate_too_long", "toasts.trailer_license_plate_too_long"],
    ["job_weight_invalid", "toasts.modify_job_weight_invalid"],
    ["trailer_repair_fields_not_found", "toasts.trailer_repair_fields_not_found"],
    ["trailer_write_verification_failed", "toasts.trailer_write_verification_error"],
    ["trailer_modal_not_found", "toasts.trailer_modal_load_error"],
  ];
  const match = mappings.find(([code]) => errorCode.includes(code));
  const key = match?.[1] || fallbackKey;
  const params = key === "toasts.trailer_license_plate_too_long"
    ? { max: TRAILER_LICENSE_PLATE_MAX_LENGTH }
    : key === "toasts.modify_job_weight_invalid"
      ? { max: JOB_WEIGHT_MAX_KG }
      : null;

  if (params) {
    showToast(key, params, "error");
  } else {
    showToast(key, "error");
  }
}

// Helper function to refresh and guard trailer actions
const trailerActionGuard = (actionName, actionFunction, options = {}) => async (...args) => {
  const { requireActiveJob = false } = options;
  console.debug(`[Trailer] ${actionName} clicked`, {
    hasProfile: Boolean(window.selectedProfilePath),
    hasSave: Boolean(window.selectedSavePath),
    hasTrailer: Boolean(window.playerTrailer),
    hasActiveJob: Boolean(window.playerTrailerHasActiveJob),
  });

  if (!window.selectedProfilePath || !window.selectedSavePath) {
    showToast("toasts.trailer_change_save_required", "warning");
    return;
  }

  console.debug(`[Trailer] ${actionName} refreshing trailer context`);
  await window.loadAllTrailers?.();

  if (window.playerTrailerLoadError) {
    console.error(`[Trailer] ${actionName} context load failed`);
    showToast("toasts.trailer_context_load_error", "error");
    return;
  }
  if (!requireActiveJob && !window.playerTrailer) {
    showToast("toasts.no_trailer_assigned_error", "warning");
    return;
  }
  if (requireActiveJob && !window.playerTrailerHasActiveJob) {
    showToast("toasts.trailer_no_active_job", "warning");
    return;
  }
  if (requireActiveJob && window.playerTrailerJobCargoMass === null) {
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

async function runGaragePlaceholder(command) {
  try {
    await invoke(command);
    showToast("toasts.garage_action_not_implemented", "warning");
  } catch (error) {
    console.error("Garage command failed:", error);
    showToast("toasts.garage_action_failed", "error");
  }
}

// --------------------------------------------------------------
// TOOL DEFINITIONS
// --------------------------------------------------------------
export const tools = {
  truck: [
    {
      title: "tools.truck.current_truck.title",
      desc: "tools.truck.current_truck.desc",
      img: "images/odometer.png",
      action: async () => {
        await openCurrentTruckModal();
      },
      disabled: false,
    },
    {
      title: "tools.truck.truck_change.title",
      desc: "tools.truck.truck_change.desc",
      img: "images/odometer.png",
      action: async () => {
        await openTruckChangeModal();
      },
      disabled: false,
    },
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
      title: "tools.trailer.trailer_change.title",
      desc: "tools.trailer.trailer_change.desc",
      img: "images/trailerRepair.jpg",
      action: async () => {
        await openTrailerChangeModal();
      },
      disabled: false,
    },
    {
      title: "tools.trailer.repair_trailer.title",
      desc: "tools.trailer.repair_trailer.desc",
      img: "images/trailerRepair.jpg",
      action: trailerActionGuard("repair", async () => {
        try {
          requireTrailerModal("#modalSlider", "repair");
          const wheelWear = Array.isArray(window.playerTrailer?.wheels_wear)
            ? window.playerTrailer.wheels_wear.filter(Number.isFinite)
            : [];
          const currentWear = Math.max(
            0,
            Number(window.playerTrailer?.body_wear) || 0,
            Number(window.playerTrailer?.chassis_wear) || 0,
            ...wheelWear
          );
          console.debug("[Trailer] opening repair modal", { currentWear });
          const shouldRepair = await openModalSlider("tools.trailer.repair_trailer.modalSliderText", 0);
          console.debug("[Trailer] repair modal result", { confirmed: Boolean(shouldRepair) });
          if (shouldRepair) {
            console.debug("[Trailer] invoking repair_player_trailer");
            const result = await invoke("repair_player_trailer");
            console.debug("[Trailer] repair_player_trailer completed", { result });
            await loadAllTrailers();
            showToast("toasts.repair_trailer_success", "success");
          }
        } catch (err) {
          console.error("Repair trailer error:", err);
          showTrailerActionError(err, "toasts.repair_trailer_error");
        }
      }),
      disabled: false,
    },
    {
      title: "tools.trailer.trailer_license_plate.title",
      desc: "tools.trailer.trailer_license_plate.desc",
      img: "images/trailer_license.jpg",
      action: trailerActionGuard("license_plate", async () => {
        try {
          const currentPlate = window.playerTrailer?.display_license_plate
            ?? extractTrailerPlateText(window.playerTrailer?.license_plate);
          requireTrailerModal("#modalText", "license_plate");
          console.debug("[Trailer] opening license plate modal", {
            currentLength: Array.from(currentPlate).length,
          });
          const newValue = await openModalText(
            "tools.trailer.trailer_license_plate.modalTextTitle",
            "tools.trailer.trailer_license_plate.modalTextPlaceholder",
            currentPlate
          );
          console.debug("[Trailer] license plate modal result", {
            submitted: newValue !== null,
            length: newValue === null ? 0 : String(newValue).trim().length,
          });
          if (newValue !== null) {
            const plate = String(newValue).trim();
            if (!plate) {
              showToast("toasts.trailer_license_plate_empty", "warning");
              return;
            }
            if (Array.from(plate).length > TRAILER_LICENSE_PLATE_MAX_LENGTH) {
              showToast(
                "toasts.trailer_license_plate_too_long",
                { max: TRAILER_LICENSE_PLATE_MAX_LENGTH },
                "warning"
              );
              return;
            }
            if (/["\\|\u0000-\u001f\u007f]/u.test(plate)) {
              showToast("toasts.trailer_license_plate_invalid", "warning");
              return;
            }
            console.debug("[Trailer] invoking set_player_trailer_license_plate", {
              length: Array.from(plate).length,
            });
            const result = await invoke("set_player_trailer_license_plate", { plate });
            console.debug("[Trailer] set_player_trailer_license_plate completed", { result });
            await loadAllTrailers();
            showToast("toasts.trailer_license_plate_success", { newValue: plate }, "success");
          }
        } catch (err) {
          console.error("Trailer license plate error:", err);
          showTrailerActionError(err, "toasts.trailer_license_plate_error");
        }
      }),
      disabled: false,
    },
    {
      title: "tools.trailer.modify_job_weight.title",
      desc: "tools.trailer.modify_job_weight.desc",
      img: "images/comingsoon.png",
      action: trailerActionGuard("job_weight", async () => {
        try {
          console.debug("[Trailer] opening job weight modal", {
            currentValue: window.playerTrailerJobCargoMass,
          });
          const { submittedValue, rawValue } = await openJobWeightModal(
            window.playerTrailerJobCargoMass
          );
          console.debug("[Trailer] job weight modal result", {
            submitted: submittedValue !== null,
          });
          if (submittedValue !== null) {
            const mass = Number(rawValue.replace(",", "."));
            if (!rawValue || !Number.isFinite(mass) || mass < 0 || mass > JOB_WEIGHT_MAX_KG) {
              showToast(
                "toasts.modify_job_weight_invalid",
                { max: JOB_WEIGHT_MAX_KG },
                "warning"
              );
              return;
            }
            console.debug("[Trailer] invoking set_player_trailer_cargo_mass", { mass });
            const result = await invoke("set_player_trailer_cargo_mass", { mass });
            console.debug("[Trailer] set_player_trailer_cargo_mass completed", { result });
            await loadAllTrailers();
            showToast("toasts.modify_job_weight_success", { newValue: mass }, "success");
          }
        } catch (err) {
          console.error("Cargo mass error:", err);
          showTrailerActionError(err, "toasts.modify_job_weight_error");
        }
      }, { requireActiveJob: true }),
      disabled: false,
    },
  ],

  profile: [
    {
      title: "tools.profile.change_xp.title",
      desc: "tools.profile.change_xp.desc",
      img: "images/xp.jpg",
      hidden: true,
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
      hidden: true,
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
  //  {
  //    title: "tools.profile.profile_sharing.title",
  //    desc: "tools.profile.profile_sharing.desc",
  //    img: "images/moveMods.png",
  //    action: async () => {
  //      openProfileSharingPage("export");
  //    },
  //    disabled: true,
  //  },
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
    {
      title: "tools.profile.level_system.title",
      desc: "tools.profile.level_system.desc",
      img: "images/xp.jpg",
      action: async () => {
        await openLevelSystemModal();
      },
    },
    {
      title: "tools.profile.mod_conflict_diagnostics.title",
      desc: "tools.profile.mod_conflict_diagnostics.desc",
      img: "images/dev.jpg",
      action: async () => {
        openModConflictDiagnosticsPage();
      },
      disabled: true,
    },
    {
      title: "tools.profile.mod_profile_manager.title",
      desc: "tools.profile.mod_profile_manager.desc",
      img: "images/moveMods.png",
      action: async () => {
        openModProfileManagerPage();
      },
      disabled: false,
    },
  ],

  garages: [
    {
      title: "tools.garages.buy.title",
      desc: "tools.garages.buy.desc",
      img: "images/comingsoon.png",
      action: async () => {
        await runGaragePlaceholder("buy_garage");
      },
    },
    {
      title: "tools.garages.upgrade.title",
      desc: "tools.garages.upgrade.desc",
      img: "images/comingsoon.png",
      action: async () => {
        await runGaragePlaceholder("upgrade_garage");
      },
    },
    {
      title: "tools.garages.buy_all.title",
      desc: "tools.garages.buy_all.desc",
      img: "images/comingsoon.png",
      action: async () => {
        await runGaragePlaceholder("buy_all_garages");
      },
    },
    {
      title: "tools.garages.relinquish.title",
      desc: "tools.garages.relinquish.desc",
      img: "images/comingsoon.png",
      action: async () => {
        await runGaragePlaceholder("relinquish_garage_ownership");
      },
    },
  ],
  settings: [
    {
      title: "editor.recovery.nav_button",
      desc: "editor.recovery.entry_summary",
      img: "images/dev.jpg",
      action: async () => {
        await window.openRecoveryCenterModal?.();
      },
      disabled: false,
    },
    {
      title: "editor.reset.title",
      desc: "editor.reset.summary",
      img: "images/money.jpg",
      action: async () => {
        await window.openSafeValueResetModal?.();
      },
      disabled: false,
    },
    {
      title: "tools.settings.user_logs.title",
      desc: "tools.settings.user_logs.desc",
      img: "images/dev.jpg",
      action: async () => {
        await window.openUserLogsModal?.();
      },
      disabled: false,
    },
    {
      title: "tools.settings.color_theme.title",
      desc: "tools.settings.color_theme.desc",
      img: "images/themeChooser.png",
      action: async () => {
        try {
          const currentTheme = localStorage.getItem("theme") || "neon-red";
          
          // Internal values map
          const themeMap = {
            "label.label_color_theme_dark": "dark",
            "label.label_color_theme_light": "light",
            "label.label_color_theme_neon": "neon",
            "label.label_color_theme_neon_red": "neon-red"
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
            document.body.classList.remove("theme-dark", "theme-light", "theme-neon", "theme-neon-red");
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
