import { tools } from "./tools.js";

/* --------------------------------------------------------------
   TOOL LOADER UND TAB HANDLING
-------------------------------------------------------------- */
const container = document.querySelector("#tool-container");
const navButtons = document.querySelectorAll(".nav-btn");
export let activeTab = "truck";

export function loadTools(tab) {
  console.log(`[app.js] Lade Tools für Tab: ${tab}`);

  activeTab = tab;
  container.innerHTML = "";

  tools[tab].forEach((t) => {
    if (t.hidden) return; // unsichtbare Tools überspringen

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

    const btn = card.querySelector("button");

    if (t.disabled) {
      btn.disabled = true;
      btn.classList.add("modal-disabled"); // CSS: rot + cursornot-allowed
      btn.textContent = "Coming Soon";
    } else {
      btn.addEventListener("click", t.action);
    }

    container.appendChild(card);
  });
}

navButtons.forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelector(".nav-btn.active")?.classList.remove("active");
    btn.classList.add("active");
    loadTools(btn.dataset.tab);
  });
});

// Default Tab
const defaultTabBtn = document.querySelector(".nav-btn.active");
if (defaultTabBtn) loadTools(defaultTabBtn.dataset.tab);

/* --------------------------------------------------------------
   MODAL REFERENCES
-------------------------------------------------------------- */
const modalText = document.querySelector("#modalText");
const modalTextTitle = document.querySelector("#modalTextTitle");
const modalTextInput = document.querySelector("#modalTextInput");
const modalTextApply = document.getElementById("modalTextApply");
const modalTextCancel = document.getElementById("modalTextCancel");

const modalNumber = document.querySelector("#modalNumber");
const modalNumberTitle = document.querySelector("#modalNumberTitle");
const modalNumberInput = document.querySelector("#modalNumberInput");
const modalNumberApply = document.getElementById("modalNumberApply");
const modalNumberCancel = document.getElementById("modalNumberCancel");

const modalSlider = document.querySelector("#modalSlider");
const modalSliderTitle = document.querySelector("#modalSliderTitle");
const modalSliderInput = document.querySelector("#modalSliderInput");
const modalSliderApply = document.getElementById("modalSliderApply");
const modalSliderCancel = document.getElementById("modalSliderCancel");

const modalMulti = document.querySelector("#modalMulti");
const modalMultiTitle = document.querySelector("#modalMultiTitle");
const modalMultiContent = document.querySelector("#modalMultiContent");
const modalMultiApplyBtn = document.getElementById("modalMultiApply");
const modalMultiCancelBtn = document.getElementById("modalMultiCancel");

/* --------------------------------------------------------------
   TEXT MODAL
-------------------------------------------------------------- */
export function openModalText(title, placeholder) {
  modalTextTitle.textContent = title;
  modalTextInput.placeholder = placeholder;
  modalText.value = "";
  modalText.style.display = "flex";

  console.log(`[app.js] Öffne Text-Modal: "${title}"`);
  return new Promise((resolve) => {
    function apply() {
      const val = modalTextInput.value;
      cleanup();
      resolve(val);
    }
    function cancel() {
      cleanup();
      resolve(null);
    }
    function cleanup() {
      modalTextApply.removeEventListener("click", apply);
      modalTextCancel.removeEventListener("click", cancel);
      modalText.style.display = "none";
    }

    modalTextApply.addEventListener("click", apply);
    modalTextCancel.addEventListener("click", cancel);
  });
};

/* --------------------------------------------------------------
   NUMBER MODAL
-------------------------------------------------------------- */
export function openModalNumber(title, value = 0) {
  modalNumberTitle.textContent = title;
  modalNumberInput.value = value;
  modalNumber.style.display = "flex";

  console.log(`[app.js] Öffne Number-Modal: "${title}" mit Wert ${value}`);
  return new Promise((resolve) => {
    function apply() {
      const val = Number(modalNumberInput.value);
      console.log("[app.js] Number-Modal 'Apply' geklickt, Wert:", val);
      cleanup();
      resolve(val);
    }
    function cancel() {
      cleanup();
      resolve(null);
    }
    function cleanup() {
      modalNumberApply.removeEventListener("click", apply);
      modalNumberCancel.removeEventListener("click", cancel);
      modalNumber.style.display = "none";
    }

    modalNumberApply.addEventListener("click", apply);
    modalNumberCancel.addEventListener("click", cancel);
  });
};

/* --------------------------------------------------------------
   SLIDER MODAL (Single 0/1)
-------------------------------------------------------------- */
export function openModalSlider(title, isChecked = 0) {
  modalSliderTitle.textContent = title;
  modalSliderInput.checked = Boolean(isChecked);
  modalSlider.style.display = "flex";

  console.log(`[app.js] Öffne Slider-Modal: "${title}" mit Wert ${isChecked}`);
  return new Promise((resolve) => {
    function apply() {
      const val = modalSliderInput.checked ? 1 : 0;
      console.log("[app.js] Slider-Modal 'Apply' geklickt, Wert:", val);
      cleanup();
      resolve(val);
    }
    function cancel() {
      cleanup();
      resolve(null);
    }
    function cleanup() {
      modalSliderApply.removeEventListener("click", apply);
      modalSliderCancel.removeEventListener("click", cancel);
      modalSlider.style.display = "none";
    }

    modalSliderApply.addEventListener("click", apply);
    modalSliderCancel.addEventListener("click", cancel);
  });
};

/* --------------------------------------------------------------
   MULTI-MODAL (NUMBER, SLIDER, DROPDOWN, ADR, CHECKBOX)
-------------------------------------------------------------- */
export function openModalMulti(title, config = []) {
  modalMultiTitle.textContent = title;
  modalMultiContent.innerHTML = "";

  console.log(`[app.js] Öffne Multi-Modal: "${title}"`);
  const adrLevels = [1, 3, 7, 15, 31, 63];

  const inputs = [];

  config.forEach((item, index) => {
    const row = document.createElement("div");
    row.className = "modal-row";

    const label = document.createElement("div");
    label.className = "modal-label";
    label.textContent = item.label;

    const control = document.createElement("div");
    control.className = "modal-control";

    /* NUMBER */
    if (item.type === "number") {
      const input = document.createElement("input");
      input.type = "number";
      input.id = item.id;
      input.value = item.value ?? 0;
      input.className = "modal-number";
      control.appendChild(input);
      inputs.push(input);
    }

    /* DROPDOWN */
    if (item.type === "dropdown") {
      const select = document.createElement("select");
      select.id = item.id;
      select.className = "modal-dropdown";

      item.options.forEach((o) => {
        const opt = document.createElement("option");
        opt.value = o;
        opt.textContent = o;
        if (String(o) === String(item.value)) opt.selected = true;
        select.appendChild(opt);
      });

      control.appendChild(select);
      inputs.push(select);
    }

    /* SLIDER / ADR */
    if (item.type === "slider" || item.type === "adr") {
      const val = document.createElement("span");
      val.id = `${item.id}_val`;
      val.className = "slider-value";

      const slider = document.createElement("input");
      slider.type = "range";

      if (item.type === "adr") {
        slider.min = 0;
        slider.max = adrLevels.length - 1;
        slider.value = adrLevels.indexOf(item.value) ?? 0;
        val.textContent = adrLevels[slider.value];

        slider.addEventListener("input", () => {
          val.textContent = adrLevels[slider.value];
        });
      } else {
        slider.min = 0;
        slider.max = 6;
        slider.value = item.value ?? 0;

        val.textContent = slider.value;


        slider.addEventListener("input", () => {
          val.textContent = slider.value;
        });
      }

      slider.id = item.id;
      slider.className = "skill-slider";
      control.appendChild(val);
      control.appendChild(slider);
      inputs.push(slider);
    }

    /* CHECKBOX */
    if (item.type === "checkbox") {
      const input = document.createElement("input");
      input.type = "checkbox";
      input.id = item.id;
      input.checked = Boolean(item.value ?? 0);
      input.className = "modal-checkbox";
      control.appendChild(input);
      inputs.push(input);
    }

    row.appendChild(label);
    row.appendChild(control);
    modalMultiContent.appendChild(row);
  });

  modalMulti.style.display = "flex";

  return new Promise((resolve) => {
    function apply() {
      const result = {};
      inputs.forEach((i) => {
        if (i.type === "range" && config.find(c => c.id === i.id)?.type === "adr") {
          const val = adrLevels[i.value];
          result[i.id] = val;
        } else if (i.type === "range" || i.type === "number") {
          result[i.id] = Number(i.value);
        } else if (i.type === "checkbox") {
          result[i.id] = i.checked ? 1 : 0;
        } else {
          result[i.id] = i.value;
        }
      });

      console.log("[app.js] Multi-Modal 'Apply' geklickt, Werte:", result);
      cleanup();
      resolve(result);
    }

    function cancel() {
      cleanup();
      resolve(null);
    }

    function cleanup() {
      modalMultiApplyBtn.removeEventListener("click", apply);
      modalMultiCancelBtn.removeEventListener("click", cancel);
      modalMulti.style.display = "none";
    }

    modalMultiApplyBtn.addEventListener("click", apply);
    modalMultiCancelBtn.addEventListener("click", cancel);
  });
};
