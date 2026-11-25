// Compatibility globals continue to serve existing tools; shared consumers read
// one live context instead of keeping their own copies of the active selection.
export const frontendState = window.frontendState ||= {
  views: new Map(),
  refresh: null,
  lastChecked: null,
  selectionBusy: false,
  get game() { return window.selectedGame || "ets2"; },
  get profilePath() { return window.selectedProfilePath || null; },
  get savePath() { return window.selectedSavePath || null; },
  get synchronization() { return window.saveSync || null; },
};

export function readPreference(key, fallback = null) {
  try { return JSON.parse(localStorage.getItem(`desktop:${key}`)) ?? fallback; }
  catch { return fallback; }
}

export function savePreference(key, value) {
  try { localStorage.setItem(`desktop:${key}`, JSON.stringify(value)); }
  catch (error) { console.warn("[desktop] preference unavailable", error); }
}

export function captureView(root) {
  const controls = [...root.querySelectorAll("input, select, textarea")].map((node, index) => ({
    id: node.id, index, value: node.value, checked: node.checked,
    focused: document.activeElement === node, start: node.selectionStart, end: node.selectionEnd,
  }));
  const positions = [...root.querySelectorAll("*")].map((node, index) => ({ index, top: node.scrollTop, left: node.scrollLeft }))
    .filter(({ top, left }) => top || left);
  const ancestors = [];
  for (let node = root; node; node = node.parentElement) ancestors.push({ node, top: node.scrollTop, left: node.scrollLeft });
  return { controls, positions, ancestors };
}

export function restoreView(root, saved) {
  if (!saved) return;
  const controls = [...root.querySelectorAll("input, select, textarea")];
  for (const value of saved.controls) {
    const node = value.id ? controls.find((control) => control.id === value.id) : controls[value.index];
    if (!node || node.type === "file" || node.type === "password") continue;
    node.value = value.value;
    if (["checkbox", "radio"].includes(node.type)) node.checked = value.checked;
    if (node.matches("input[type='search'], [data-garage-search], [data-garage-filter]")) {
      node.dispatchEvent(new Event(node.tagName === "SELECT" ? "change" : "input", { bubbles: true }));
    }
    if (value.focused) {
      node.focus({ preventScroll: true });
      if (typeof value.start === "number" && ["text", "search", "url", "tel", "password"].includes(node.type)) node.setSelectionRange(value.start, value.end);
    }
  }
  const descendants = root.querySelectorAll("*");
  for (const value of saved.positions) {
    const node = descendants[value.index];
    if (node) { node.scrollTop = value.top; node.scrollLeft = value.left; }
  }
  for (const value of saved.ancestors) {
    if (value.node.isConnected) { value.node.scrollTop = value.top; value.node.scrollLeft = value.left; }
  }
}

// Coalesces identical concurrent reads; a changed key starts after the current
// request. Callers still validate their context before committing the response.
export function singleFlight(task, key = () => "default") {
  let running = null;
  let runningKey;
  let next = null;
  return function run(...args) {
    const requestedKey = key(...args);
    if (running) {
      if (requestedKey === runningKey) return running;
      if (next?.key === requestedKey) return next.promise;
      const promise = running.catch(() => {}).then(() => run(...args));
      next = { key: requestedKey, promise };
      return promise;
    }
    runningKey = requestedKey;
    running = Promise.resolve().then(() => task(...args)).finally(() => { running = null; next = null; });
    return running;
  };
}
