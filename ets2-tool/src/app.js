const container = document.querySelector("#tool-container");
const navButtons = document.querySelectorAll(".nav-btn");
const linkButtons = document.querySelectorAll(".link-btn");

function loadTools(tab) {
    container.innerHTML = "";

    tools[tab].forEach(t => {
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

// default
loadTools("truck");

// NEUE SELEKTOREN FÜR JEDES MODAL
// TEXT MODAL
const modalText = document.querySelector("#modalText");
const modalTextTitle = document.querySelector("#modalTextTitle");
const modalTextInput = document.querySelector("#modalTextInput");
const modalTextCancel = document.querySelector("#modalTextCancel");
const modalTextApply = document.querySelector("#modalTextApply");

// Number Modal
const modalNumber = document.querySelector("#modalNumber");
const modalNumberTitle = document.querySelector("#modalNumberTitle");
const modalNumberInput = document.querySelector("#modalNumberInput");
const modalNumberCancel = document.querySelector("#modalNumberCancel");
const modalNumberApply = document.querySelector("#modalNumberApply");

// Slider Modal
const modalSlider = document.querySelector("#modalSlider");
const modalSliderTitle = document.querySelector("#modalSliderTitle");
const modalSliderInput = document.querySelector("#modalSliderInput");
const modalSliderCancel = document.querySelector("#modalSliderCancel")
const modalSliderApply = document.querySelector("#modalSliderApply");

// --- Funktion zum Öffnen ---

function openModalText(title, placeholder) {
    modalTextTitle.textContent = title;
    modalTextInput.placeholder = placeholder;
    modalText.style.display = "flex";
}

function openModalNumber(title, initialValue) {
    modalNumberTitle.textContent = title;
    modalNumberInput.value = initialValue || 0;
    modalNumber.style.display = "flex";
}

function openModalSlider(title, isChecked) {
    modalSliderTitle.textContent = title;
    modalSliderInput.checked = isChecked;
    modalSlider.style.display = "flex";
}

// --- Handhabung zur Schliessung der Modale 

// Close Buttons 

modalTextApply.onclick = () => {
    modalText.style.display = "none";
    console.log("Angewendet:", modalTextInput.value);
}

modalNumberApply.onclick = () => {
    modalNumber.style.display = "none";
    console.log("Angewendete Nummer:", modalNumberInput.value);
}

modalSliderApply.onclick = () => {
    modalSlider.style.display = "none";
    console.log("Apply Button used", modalSliderInput.value)
}

modalTextCancel.onclick = () => {
    modalText.style.display = "none";
    console.log("Angewendet -> Close Button", modalTextCancel.value);
}

modalNumberCancel.onclick = () => {
    modalNumber.style.display = "none";
    console.log("Angewendet -> Close Button", modalNumberCancel.value);
}

modalSliderCancel.onclick = () => {
    modalSlider.style.display = "none";
    console.log("Angewendet - Close Button", modalSliderCancel.value);
}



// tab switching
navButtons.forEach(btn => {
    btn.onclick = () => loadTools(btn.dataset.tab);
});
