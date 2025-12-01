// --------------------------------------------------------------
// LOAD TOOLS INTO UI (Ihr bestehender Code)
// --------------------------------------------------------------
const container = document.querySelector("#tool-container");
const navButtons = document.querySelectorAll(".nav-btn"); // Diese Variable wird jetzt genutzt

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

// --------------------------------------------------------------
// NEUER CODE: Event-Listener für Nav-Buttons
// --------------------------------------------------------------

navButtons.forEach(button => {
    button.addEventListener("click", function() {
        // 1. "active" Klasse vom aktuell aktiven Button entfernen
        document.querySelector(".nav-btn.active").classList.remove("active");
        
        // 2. "active" Klasse zum geklickten Button hinzufügen
        this.classList.add("active"); // 'this' bezieht sich auf den Button, der geklickt wurde

        // 3. Den entsprechenden Tab-Inhalt laden
        const tabToLoad = this.dataset.tab;
        loadTools(tabToLoad);
    });
});


// Default load (von der aktiven Klasse) - Ihr bestehender Code
const defaultTabBtn = document.querySelector(".nav-btn.active");
if (defaultTabBtn) {
    loadTools(defaultTabBtn.dataset.tab);
} 


// --------------------------------------------------------------
// MODAL REFERENCES
// --------------------------------------------------------------
const modalText = document.querySelector("#modalText");
const modalTextTitle = document.querySelector("#modalTextTitle");
const modalTextInput = document.querySelector("#modalTextInput");

const modalNumber = document.querySelector("#modalNumber");
const modalNumberTitle = document.querySelector("#modalNumberTitle");
const modalNumberInput = document.querySelector("#modalNumberInput");

const modalSlider = document.querySelector("#modalSlider");
const modalSliderTitle = document.querySelector("#modalSliderTitle");
const modalSliderInput = document.querySelector("#modalSliderInput");

const modalMulti = document.querySelector("#modalMulti");
const modalMultiTitle = document.querySelector("#modalMultiTitle");
const modalMultiContent = document.querySelector("#modalMultiContent");

// --------------------------------------------------------------
// TEXT MODAL
// --------------------------------------------------------------
window.openModalText = function (title, placeholder) {
    modalTextTitle.textContent = title;
    modalTextInput.placeholder = placeholder;
    modalText.style.display = "flex";
};

document.querySelector("#modalTextCancel").onclick = () =>
    (modalText.style.display = "none");

document.querySelector("#modalTextApply").onclick = () => {
    console.log("Text applied:", modalTextInput.value);
    modalText.style.display = "none";
};

// --------------------------------------------------------------
// NUMBER MODAL
// --------------------------------------------------------------
window.openModalNumber = function (title, value) {
    modalNumberTitle.textContent = title;
    modalNumberInput.value = value || 0;
    modalNumber.style.display = "flex";
};

document.querySelector("#modalNumberCancel").onclick = () =>
    (modalNumber.style.display = "none");

document.querySelector("#modalNumberApply").onclick = () => {
    console.log("Number applied:", modalNumberInput.value);
    modalNumber.style.display = "none";
};

// --------------------------------------------------------------
// SLIDER MODAL
// --------------------------------------------------------------
window.openModalSlider = function (title, isChecked) {
    modalSliderTitle.textContent = title;
    modalSliderInput.checked = Boolean(isChecked);
    modalSlider.style.display = "flex";
};

document.querySelector("#modalSliderCancel").onclick = () =>
    (modalSlider.style.display = "none");

document.querySelector("#modalSliderApply").onclick = () => {
    console.log("Slider applied:", modalSliderInput.checked);
    modalSlider.style.display = "none";
};

// --------------------------------------------------------------
// MULTI-MODAL (für mehrere Slider/Dropdown/Number Inputs)
// --------------------------------------------------------------
window.openModalMulti = function (title, fields) {
    modalMultiTitle.textContent = title;
    modalMultiContent.innerHTML = "";

    fields.forEach((f) => {
        const row = document.createElement("div");
        row.classList.add("multi-row");

        if (f.type === "slider") {
            row.innerHTML = `
                <label>${f.label}</label>
                <input type="checkbox" id="${f.id}" ${f.value ? "checked" : ""}>
            `;
        } else if (f.type === "dropdown") {
            row.innerHTML = `
                <label>${f.label}</label>
                <select id="${f.id}">
                    ${f.options.map(o => `<option value="${o}" ${f.value === o ? "selected" : ""}>${o}</option>`).join("")}
                </select>
            `;
        } else if (f.type === "number") {
            row.innerHTML = `
                <label>${f.label}</label>
                <input type="number" id="${f.id}" value="${f.value}">
            `;
        }

        modalMultiContent.appendChild(row);
    });

    modalMulti.style.display = "flex";
};

document.querySelector("#modalMultiCancel").onclick = () => (modalMulti.style.display = "none");

document.querySelector("#modalMultiApply").onclick = () => {
    const inputs = modalMultiContent.querySelectorAll("input, select");
    const values = {};

    inputs.forEach((inp) => {
        values[inp.id] = inp.type === "checkbox" ? inp.checked : inp.value;
    });

    console.log("MULTI MODAL VALUES:", values);
    modalMulti.style.display = "none";
};

// --------------------------------------------------------------
// TAB SWITCHING
// --------------------------------------------------------------
navButtons.forEach((btn) => {
    btn.onclick = () => {
        navButtons.forEach((b) => b.classList.remove("active"));
        btn.classList.add("active");
        loadTools(btn.dataset.tab);
    };
});
