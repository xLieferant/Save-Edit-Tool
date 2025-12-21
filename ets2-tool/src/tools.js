import {
  openModalNumber,
  openModalText,
  openModalSlider,
  openModalMulti,
} from "./app.js";

// --------------------------------------------------------------
// TOOL DEFINITIONS
// --------------------------------------------------------------
export const tools = {
  truck: [
    {
      title: "Repair Truck",
      desc: "Repair your current truck",
      img: "images/repair.png",
      //action: () => openModalSlider("Repair Truck", false),
      action: () => {},
      disabled: true,
    },
    {
      title: "Fuel Level - 'COMING SOON'",
      desc: "Change your fuel level at your current truck",
      img: "images/gasstation.jpg",
      // action: () =>
      //   openModalNumber(
      //     "Change fuel level",
      //     window.playerTruck?.trip_fuel_l || 0
      //   ),

      action: () => {},
      disabled: true,
    },
    {
      title: "Truck Mileage",
      desc: "Change your Mileage at your current truck",
      img: "images/odometer.png",
      action: async () => {
        const newValue = await openModalNumber(
          "Change your odometer",
          window.playerTruck?.odometer || 0
        );
        if (newValue !== null) {
          await invoke("edit_truck_odometer", { value: newValue });
          await loadAllTrucks();
        }
      },
    },
    {
      title: "Truck License Plate",
      desc: "Change your license plate",
      img: "images/trailer_license.jpg",
      action: async () => {
        const newValue = await openModalText(
          "Change your license plate",
          window.playerTruck?.license_plate || ""
        );
        if (newValue !== null) {
          await invoke("edit_truck_license_plate", { value: newValue });
          await loadAllTrucks();
        }
      },
    },
  ],

  trailer: [
    {
      // [] TO DO | Trailer HP finden
      title: "Repair - 'COMING SOON'",
      desc: "Repair your Trailer",
      img: "images/trailerRepair.jpg",
      action: () => {}, // keine Aktion
      // action: () => openModalSlider("Repair Trailer", false),
      disabled: true,
    },
    {
      // [] TO DO. Kennzeichen angeben
      title: "Change Trailer License Plate - 'COMING SOON'",
      desc: "Modify your trailer license plate",
      img: "images/trailer_license.jpg",
      // action: () =>
      //   openModalText("Change trailer license", "New License Plate"),
      action: () => {},
      disabled: true,
    },
    {
      // [] TO DO Job Weight finden
      title: "Modify Job Weight - 'COMING SOON'",
      desc: "Adjust the job's cargo weight",
      img: "images/comingsoon.png",
      //action: () => openModalNumber("Modify job weight", "Weight in kg"),
      action: () => {},
      disabled: true,
    },
  ],

  profile: [
    {
      title: "Change XP",
      desc: "Modify profile XP",
      img: "images/xp.jpg",
      action: async () => {
        const newValue = await openModalNumber(
          "Change experience",
          window.currentProfileData?.xp || 0
        );
        if (newValue !== null) {
          await window.invoke("apply_setting", { key: "xp", value: newValue });
          await loadProfileData();
        }
      },
    },
    {
      title: "Money",
      desc: "Modify users Money",
      img: "images/money.jpg",
      action: async () => {
        const newValue = await openModalNumber(
          "Change money",
          window.currentProfileData?.money || 0
        );
        if (newValue !== null) {
          await window.invoke("apply_setting", { key: "money", value: newValue });
          await loadProfileData();
        }
      },
    },
    {
      title: "Experience Skills",
      desc: "Set skill points",
      img: "images/skillPoint.jpg",
      action: async () => {
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
          }, // mechanical = eco!
        ]);
        if (res) {
          // Hier musst du noch die invoke-Befehle für die Skills hinzufügen
          // z.B. await invoke("edit_skill", { skill: 'adr', value: res.skill_adr });
          console.log("Skills to save:", res);
          await loadQuicksave(); // Daten neu laden
        }
      },
    },
    {
      // hinzugefügt für Account Stats
      title: "Stats",
      desc: "Account informations",
      img: "images/skillPoint.jpg", // <- Muss noch geändert werden
      action: async () => {
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
        }
      },
    },
  ],

  settings: [
    {
      // [] TO DO, value ist nicht .baseConfig! Muss noch geändert werden
      title: "Color Theme - 'COMING SOON'",
      desc: "Change the UI theme",
      img: "images/comingsoon.png",
      action: () =>
        openModalMulti("Choose Color Theme", [
          {
            type: "dropdown",
            id: "theme",
            label: "Theme",
            value: window.baseConfig?.theme || "Dark", // hier muss der value noch geändert  werden!
            options: ["Dark", "Light", "Neon"],
          },
        ]),
    },
    {
      title: "Convoy 128",
      desc: "Change convoy size",
      img: "images/convoy.jpg",
      action: async () => {
        const newValue = await openModalNumber(
          "Convoy Size",
          window.baseConfig?.max_convoy_size || 8
        );
        if (newValue !== null) {
          await window.invoke("apply_setting", { key: "max_convoy_size", value: newValue });
          await loadBaseConfig();
        }
      },
    },
    {
      // [] TO DO, value ist nicht .baseConfig! Muss noch geändert werden
      title: "Language - 'COMING SOON'",
      desc: "Change your language",
      img: "images/language.png",
      // action: async () => {
      //   const res = await openModalMulti("Language Settings", [
      //     {
      //       type: "dropdown",
      //       id: "languageSelector",
      //       label: "Language",
      //       value: window.baseConfig?.language || "Deutsch",
      //       options: [
      //         "Deutsch",
      //         "English (Coming Soon)",
      //         "Spanish (Coming Soon)",
      //         "French (Coming Soon)",
      //         "Italian (Coming Soon)",
      //       ],
      //     },
      //   ]);
      //   if (res) {
      //      // ... Speicherlogik
      //   }
      // },
      action: () => {},
      disabled: true,
    },
    {
      title: "Traffic value",
      desc: "Change the traffic factor",
      img: "images/traffic_value.png", // <- Bild muss noch eingefügt werden!
      action: async () => {
        const newValue = await openModalNumber(
          "g_traffic",
          window.baseConfig?.traffic || 1
        );
        if (newValue !== null) {
          await window.invoke("apply_setting", { key: "traffic", value: newValue });
          await loadBaseConfig();
        }
      },
    },
    {
      title: "Parking Doubles",
      desc: "Do you want to park double trailer?",
      img: "images/parking_double.png", // <- Parking double Bilder einfügen
      action: async () => {
        const newValue = await openModalSlider(
          "Do you want to park doubles?",
          window.readSaveGameConfig?.factor_parking_doubles || 0
        );
        if (newValue !== null) {
          await invoke("edit_save_config_value", { key: "g_simple_parking_doubles", value: String(newValue) });
          await loadProfileSaveConfig();
        }
      },
    },
    {
      title: "Dev Mode",
      desc: "Developer & Console Mode",
      img: "images/dev.jpg",
      action: async () => {
      const res = await openModalMulti("Developer Settings", [
        { type: "checkbox", id: "developer", label: "Developer", value: window.baseConfig?.developer },
        { type: "checkbox", id: "console", label: "Console", value: window.baseConfig?.console },
      ]);

      if (res) {
        await window.invoke("apply_setting", { key: "developer", value: res.developer });
        await window.invoke("apply_setting", { key: "console", value: res.console });
        await loadBaseConfig();
      }
    },
    },
  ],
};
