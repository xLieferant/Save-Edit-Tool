// Tool categories
const tools = {
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
        }
    ]
};
