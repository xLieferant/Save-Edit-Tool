// Tool categories
const tools = {

    truck: [
        {
            title: "Repair Truck",
            desc: "Repair your current truck",
            img: "images/xxx.jpg"
        },
        {
            title: "Fuel Level",
            desc: "Change your fuel level at your current truck",
            img: "images/fuel.jpg",
            action: () => openModal("Change fuel level","How much fuel do you want?")
        },
        {
            title: "Truck milage",
            desc: "Change your Milage at your current truck",
            img: "images/odometer.jpg",
            action: () => openModal("Change your odometer","How much KM do you want?")
        }
    ],

    trailer: [
        {
            title: "Repair",
            desc: "Repair your Trialer",
            img: "images/trailerRepair.jpg",
            action: () => openModal("","")
        },
        {
            title: "Change Trailer License Plate",
            desc: "Modify the trailer license plate",
            img: "images/trailer_license.jpg",
            action: () => openModal("Change license plate", "Enter new plate")
        },
        {
            title: "Modify Job Weight",
            desc: "Adjust the job's cargo weight",
            img: "images/job_weight.jpg",
            action: () => openModal("Modify job weight", "Enter weight")
        }
    ],

    profile: [
        {
            title: "Change XP",
            desc: "Modify profile XP",
            img: "images/xp.jpg",
            action: () => openModal("Change experience", "Enter experience")
        },
        {
            title: "Money",
            desc: "Modify users Money",
            img: "images/money.jpg",
            action: () => openModal("Change money")
        },
        {
            title: "Experience Skills",
            desc: "Set your Skill Points",
            img: "images/skillPoint.jpg",
            action: () => openModal("Set Skill points")
        }
    ],
    settings: [
        {
            title: "Color Theme",
            desc: "Change the Style",
            img: "images/styles.jpg",
            action: () => openModal("","")
        },
        {
            title: "Convoy 128",
            desc: "change your Config to 128",
            img: "images/convoy.jpg",
            action: () => openModal("","")
        },
        {
            title: "Language",
            desc: "change your language",
            img: "images/lang.jpg",
            action: () => openModal("","")
        },
        {
            title: "Activate Dev. mode",
            desc: "Activate the develepmont mode",
            img: "images/dev.jpg",
            action: () => openModal("","")
        },
        {
            title: "Save Document folders",
            desc: "Where are your saves?",
            img: "images/Save.jpg",
            action: () => openModal("","")
        }
        
    ],
};
