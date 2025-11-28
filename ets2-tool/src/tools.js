// Tool categories
const tools = {

    truck: [
        {
            title: "Fuel Level",
            desc: "Change your fuel level at your current truck",
            img: "images/fuel.jpg",
            action: () => openModal("Change fuel level","How much fuel do you want?")
        }
    ],

    trailer: [
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
        }
    ],
    settings: [
        {
            title: "Convoy 128",
            desc: "change your Config to 128",
            img: "images/convoy.jpg",
            action: () => openModal("","")
        }
    ],
};
