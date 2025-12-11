// --------------------------------------------------------------
// TOOL DEFINITIONS
// --------------------------------------------------------------
const tools = {
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
      action: () =>
        openModalNumber(
          "Change your odometer",
          window.playerTruck?.odometer || 0
        ),
    },
    {
      title: "Truck License Plate",
      desc: "Change your license plate",
      img: "images/trailer_license.jpg",
      action: () =>
        openModalText(
          "Change your license plate",
          window.playerTruck?.license_plate || ""
        ),
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
      action: () =>
        openModalNumber(
          "Change experience",
          window.currentProfileData?.xp || 0
        ),
    },
    {
      title: "Money",
      desc: "Modify users Money",
      img: "images/money.jpg",
      action: () =>
        openModalNumber("Change money", window.currentProfileData?.money || 0),
    },
    {
      title: "Experience Skills",
      desc: "Set skill points",
      img: "images/skillPoint.jpg",
      action: () =>
        openModalMulti("Set Experience Skills", [
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
        ]),
    },
    {
      // hinzugefügt für Account Stats
      title: "Stats",
      desc: "Account informations",
      img: "images/skillPoint.jpg", // <- Muss noch geändert werden
      action: () =>
        openModalMulti("Show differnet stats!", [
          {
            type: "number",
            id: "skill_long",
            label: "Recruitment Centers",
            value: window.currentProfileData.recruitments || 0,
          },
          {
            type: "number",
            id: "skill_long",
            label: "Dealers",
            value: window.currentProfileData.dealers || 0,
          },
          {
            type: "number",
            id: "skill_long",
            label: "Visited cities",
            value: window.currentProfileData.visited_cities || 0,
          },
        ]),
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
      action: () =>
        openModalNumber("Convoy Size", window.baseConfig?.max_convoy_size || 8),
    },
    {
      // [] TO DO, value ist nicht .baseConfig! Muss noch geändert werden
      title: "Language - 'COMING SOON'",
      desc: "Change your language",
      img: "images/language.png",
      // action: () =>
      //   openModalMulti("Language Settings", [
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
      //   ]),
      action: () => {},
      disabled: true,
    },
    {
      title: "Traffic value",
      desc: "Change the traffic factor",
      img: "images/traffic_value.png", // <- Bild muss noch eingefügt werden!
      action: () =>
        openModalNumber("g set_traffic", window.baseConfig?.traffic || 1), // <- 1 ist Standard Value
    },
    {
      title: "Parking Doubles",
      desc: "Do you want to park double trailer?",
      img: "images/parking_double.png", // <- Parking double Bilder einfügen
      action: () =>
        openModalSlider(
          "Do you want to park doubles?",
          window.readSaveGameConfig?.factor_parking_doubles || 0
        ), // <-- 0 Standard wert
    },
    {
      title: "Dev Mode",
      desc: "Developer & Console Mode",
      img: "images/dev.jpg",
      action: () =>
        openModalMulti("Developer Settings", [
          {
            type: "checkbox",
            id: "developer",
            label: "Developer Mode",
            value: window.baseConfig?.developer || false,
          },
          {
            type: "checkbox",
            id: "console",
            label: "Console Mode",
            value: window.baseConfig?.console || false,
          },
        ]).then((result) => {
          if (result) {
            window.baseConfig.developer = result.developer;
            window.baseConfig.console = result.console;
          }
        }),
    },
  ],
};
