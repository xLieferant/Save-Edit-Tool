// --------------------------------------------------------------
// TOOL DEFINITIONS
// --------------------------------------------------------------
const tools = {
  truck: [
    {
      title: "Repair Truck",
      desc: "Repair your current truck",
      img: "images/repair.png",
      action: () => openModalSlider("Repair Truck", false),
    },
    {
      title: "Fuel Level",
      desc: "Change your fuel level at your current truck",
      img: "images/gasstation.jpg",
      action: () => openModalNumber("Change fuel level", "How much fuel?"),
    },
    {
      title: "Truck milage",
      desc: "Change your Milage at your current truck",
      img: "images/odometer.png",
      action: () => openModalNumber("Change your odometer", "How many KM?"),
    },
  ],

  trailer: [
    {
      title: "Repair",
      desc: "Repair your Trailer",
      img: "images/trailerRepair.jpg",
      action: () => openModalSlider("Repair Trailer", false),
    },
    {
      title: "Change Trailer License Plate",
      desc: "Modify your trailer license plate",
      img: "images/trailer_license.jpg",
      action: () =>
        openModalText("Change trailer license", "New License Plate"),
    },
    {
      title: "Modify Job Weight",
      desc: "Adjust the job's cargo weight",
      img: "images/job_weight.jpg",
      action: () => openModalNumber("Modify job weight", "Weight in kg"),
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
      action: () => openModalText("Set Skill points", "Enter skill value"),
    },
  ],

  settings: [
    {
      title: "Color Theme",
      desc: "Change the UI theme",
      img: "images/styles.jpg",
      action: () =>
        openModalMulti("Choose Color Theme", [
          {
            type: "dropdown",
            id: "theme",
            label: "Theme",
            value: window.baseConfig?.theme || "Dark",
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
      title: "Language",
      desc: "Change your language",
      img: "images/lang.jpg",
      action: () =>
        openModalMulti("Language Settings", [
          {
            type: "dropdown",
            id: "languageSelector",
            label: "Language",
            value: window.baseConfig?.language || "Deutsch",
            options: ["Deutsch", "English", "Spanish", "French", "Italian"],
          },
        ]),
    },
    {
      title: "Dev Mode",
      desc: "Developer & Console Mode",
      img: "images/dev.jpg",
      action: () =>
        openModalMulti("Developer Settings", [
          {
            type: "slider",
            id: "developer",
            label: "Developer Mode",
            value: window.baseConfig?.developer || false,
          },
          {
            type: "slider",
            id: "console",
            label: "Console Mode",
            value: window.baseConfig?.console || false,
          },
        ]),
    },
  ],
};
