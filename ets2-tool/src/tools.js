// Tool categories
const tools = {

    truck: [
        {
            title: "Repair Truck",
            desc: "Repair your current truck",
            img: "images/repair.jpg",
            action: () => openModalSlider("", "")
        },
        {
            title: "Fuel Level",
            desc: "Change your fuel level at your current truck",
            img: "images/gasstation.jpg",
            action: () => openModalNumber("Change fuel level","How much fuel do you want?")
        },
        {
            title: "Truck milage",
            desc: "Change your Milage at your current truck",
            img: "images/odometer.jpg",
            action: () => openModalNumber("Change your odometer","How much KM do you want?")
        }
    ],

    trailer: [
        {
            title: "Repair",
            desc: "Repair your Trialer",
            img: "images/trailerRepair.jpg",
            action: () => openModalSlider("","")
        },
        {
            title: "Change Trailer License Plate",
            desc: "Modify the trailer license plate",
            img: "images/trailer_license.jpg",
            action: () => openModalText("Change license plate", "Enter new plate")
        },
        {
            title: "Modify Job Weight",
            desc: "Adjust the job's cargo weight",
            img: "images/job_weight.jpg",
            action: () => openModalNumber("Modify job weight", "Enter weight")
        }
    ],

    profile: [
        {
            title: "Change XP",
            desc: "Modify profile XP",
            img: "images/xp.jpg",
            action: () => openModalNumber("Change experience", window.currentProfileData?.xp || 0)
        },
        {
            title: "Money",
            desc: "Modify users Money",
            img: "images/money.jpg",
            action: () => openModalNumber("Change money", window.currentProfileData?.money || 0)
        },
        {
            title: "Experience Skills",
            desc: "Set your Skill Points",
            img: "images/skillPoint.jpg",
            action: () => openModalText("Set Skill points")
        }
    ],
    settings: [
        {
            title: "Color Theme",
            desc: "Change the Style",
            img: "images/styles.jpg",
            action: () => openModalSlider("","")
        },
        {
            title: "Convoy 128",
            desc: "change your Config to 128",
            img: "images/convoy.jpg",
            action: () => openModalSlider("","")
        },
        {
            title: "Language",
            desc: "change your language",
            img: "images/lang.jpg",
            action: () => openModalSlider("","")
        },
        {
            title: "Activate Dev. mode",
            desc: "Activate the develepmont mode",
            img: "images/dev.jpg",
            action: () => openModalSlider("","")
        },
        {
            title: "Save Document folders",
            desc: "Where are your saves?",
            img: "images/Save.jpg",
            action: () => openModalText("","")
        }
        
    ],
};
