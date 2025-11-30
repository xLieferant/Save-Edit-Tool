const container = document.querySelector("#tool-container");
const navButtons = document.querySelectorAll(".nav-btn");

let activeTab = "truck";

function loadTools(tab) {
    activeTab = tab;
    container.innerHTML = "";

    tools[tab].forEach((t) => {
        const card = document.createElement("div");
        card.classList.add("tool-card");

        card.innerHTML = `
            <img src="${t.img}">
            <div class="tool-content">
                <h3>${t.title}</h3>
                <p>${t.desc}</p>
                <button>Open</button>
            </div>
        `;

        card.querySelector("button").addEventListener("click", t.action);
        container.appendChild(card);
    });
}

// default load
loadTools("truck");

// Modal references
const modalText = document.querySelector("#modalText");
const modalTextTitle = document.querySelector("#modalTextTitle");
const modalTextInput = document.querySelector("#modalTextInput");

const modalNumber = document.querySelector("#modalNumber");
const modalNumberTitle = document.querySelector("#modalNumberTitle");
const modalNumberInput = document.querySelector("#modalNumberInput");

const modalSlider = document.querySelector("#modalSlider");
const modalSliderTitle = document.querySelector("#modalSliderTitle");
const modalSliderInput = document.querySelector("#modalSliderInput");

// OPEN MODALS
window.openModalText = function(title, placeholder) {
    modalTextTitle.textContent = title;
    modalTextInput.placeholder = placeholder;
    modalText.style.display = "flex";
};

window.openModalNumber = function(title, placeholderOrValue) {
    modalNumberTitle.textContent = title;

    if (typeof placeholderOrValue === "string") {
        modalNumberInput.placeholder = placeholderOrValue;
        modalNumberInput.value = "";
    } else {
        modalNumberInput.value = placeholderOrValue || 0;
    }

    modalNumber.style.display = "flex";
};

window.openModalSlider = function(title, isChecked) {
    modalSliderTitle.textContent = title;
    modalSliderInput.checked = Boolean(isChecked);
    modalSlider.style.display = "flex";
};

// CLOSE events
document.querySelector("#modalTextCancel").onclick = () => (modalText.style.display = "none");
document.querySelector("#modalNumberCancel").onclick = () => (modalNumber.style.display = "none");
document.querySelector("#modalSliderCancel").onclick = () => (modalSlider.style.display = "none");

document.querySelector("#modalTextApply").onclick = () => {
    modalText.style.display = "none";
    console.log("Text applied:", modalTextInput.value);
};

document.querySelector("#modalNumberApply").onclick = () => {
    modalNumber.style.display = "none";
    console.log("Number applied:", modalNumberInput.value);
};

document.querySelector("#modalSliderApply").onclick = () => {
    modalSlider.style.display = "none";
    console.log("Slider applied:", modalSliderInput.checked);
};

// Tab switching
navButtons.forEach((btn) => {
    btn.onclick = () => {
        navButtons.forEach(b => b.classList.remove("active"));
        btn.classList.add("active");
        loadTools(btn.dataset.tab);
    };
});
