import { t } from "./i18n.js";

export async function addPickerSearch(list, key) {
  if (!list || list.querySelector(".picker-search")) return;
  const input = document.createElement("input");
  input.type = "search";
  input.className = "picker-search";
  input.dataset.translatePlaceholder = key;
  input.dataset.translateAriaLabel = key;
  input.value = list.dataset.search || "";
  const filter = () => {
    list.dataset.search = input.value;
    const query = input.value.trim().toLocaleLowerCase();
    for (const option of list.querySelectorAll("[role='option']")) option.hidden = !option.textContent.toLocaleLowerCase().includes(query);
    for (const section of list.querySelectorAll(".dropdown-section")) section.hidden = !section.querySelector("[role='option']:not([hidden])");
  };
  input.addEventListener("input", filter);
  list.prepend(input);
  filter();
  const label = await t(key);
  input.placeholder = label;
  input.setAttribute("aria-label", label);
  // The picker itself is a keyboard trigger; typing inside must not toggle it.
  input.addEventListener("keydown", (event) => {
    if (["Enter", " "].includes(event.key)) event.stopPropagation();
  });
  if (!list.dataset.searchKeyboard) {
    list.dataset.searchKeyboard = "true";
    list.addEventListener("keydown", (event) => {
      if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
      const options = [...list.querySelectorAll("[role='option']:not([hidden])")];
      if (!options.length) return;
      event.preventDefault();
      event.stopPropagation();
      const index = options.indexOf(document.activeElement);
      const next = event.key === "Home" ? 0 : event.key === "End" ? options.length - 1
        : (index + (event.key === "ArrowDown" ? 1 : -1) + options.length) % options.length;
      options[next].focus();
    });
  }
}
