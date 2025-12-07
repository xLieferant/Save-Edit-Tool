/* --------------------------------------------------------------
   TOOL LOADER UND TAB HANDLING
-------------------------------------------------------------- */
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

const modalNumber = document.querySelector("#modalNumber");
const modalNumberTitle = document.querySelector("#modalNumberTitle");
const modalNumberInput = document.querySelector("#modalNumberInput");

const modalSlider = document.querySelector("#modalSlider");
const modalSliderTitle = document.querySelector("#modalSliderTitle");
const modalSliderInput = document.querySelector("#modalSliderInput");

const modalMulti = document.querySelector("#modalMulti");
const modalMultiTitle = document.querySelector("#modalMultiTitle");
const modalMultiContent = document.querySelector("#modalMultiContent");

const modalMultiApplyBtn = document.getElementById("modalMultiApply");
const modalMultiCancelBtn = document.getElementById("modalMultiCancel");

/* --------------------------------------------------------------
   TEXT MODAL
-------------------------------------------------------------- */
window.openModalText = function (title, placeholder) {
  modalTextTitle.textContent = title;
  modalTextInput.placeholder = placeholder;
  modalText.style.display = "flex";

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
window.openModalNumber = function (title, value = 0) {
  modalNumberTitle.textContent = title;
  modalNumberInput.value = value;
  modalNumber.style.display = "flex";

  return new Promise((resolve) => {
    function apply() {
      const val = Number(modalNumberInput.value);
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
   SLIDER MODAL
-------------------------------------------------------------- */
window.openModalSlider = function (title, isChecked) {
  modalSliderTitle.textContent = title;
  modalSliderInput.checked = Boolean(isChecked);
  modalSlider.style.display = "flex";

  return new Promise((resolve) => {
    function apply() {
      const val = modalSliderInput.checked;
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
   MULTI-MODAL (NUMBER, SLIDER, DROPDOWN, ADR)
-------------------------------------------------------------- */
window.openModalMulti = function (title, config = []) {
  modalMultiTitle.textContent = title;
  modalMultiContent.innerHTML = "";

  config.forEach((item) => {
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
    }

    /* SLIDER */
    if (item.type === "slider" || item.type === "adr") {
      const val = document.createElement("span");
      val.id = `${item.id}_val`;
      val.className = "slider-value";

      let slider = document.createElement("input");
      slider.type = "range";

      if (item.type === "adr") {
        // Mapping für ADR Levels
        const adrLevels = [1, 3, 7, 15, 31, 63];
        slider.min = 0;
        slider.max = adrLevels.length - 1;
        slider.value = adrLevels.indexOf(item.value) ?? 0; // Setze Startwert auf Index
        val.textContent = adrLevels[slider.value];

        slider.addEventListener("input", () => {
          val.textContent = adrLevels[slider.value];
        });
      } else {
        // Normaler Slider
        slider.min = 0;
        slider.max = 6;
        slider.value = item.value ?? 0;

        slider.addEventListener("input", () => {
          val.textContent = slider.value;
        });
      }

      slider.id = item.id;
      slider.className = "skill-slider";

      control.appendChild(val);
      control.appendChild(slider);
    }

    row.appendChild(label);
    row.appendChild(control);

    modalMultiContent.appendChild(row);
  });

  modalMulti.style.display = "flex";

  return new Promise((resolve) => {
    function apply() {
      const inputs = modalMultiContent.querySelectorAll("input, select");
      const result = {};

      inputs.forEach((i) => {
        if (i.type === "range" || i.type === "number") {
          result[i.id] = Number(i.value);
        } else {
          result[i.id] = i.value;
        }
      });

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
