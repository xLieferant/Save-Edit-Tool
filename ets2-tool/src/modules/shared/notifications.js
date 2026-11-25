import { t } from "./i18n.js";

// All producers share one normal-flow stack. Timers start when an item is visible.
const entries = new Map();
const queue = [];
const visible = [];
let container;
let disposed = false;
const storageKey = "ets2_read_news_v1";
const readNews = () => {
  try { return JSON.parse(localStorage.getItem(storageKey) || "[]"); }
  catch { return []; }
};

function stack() {
  if (!container) {
    container = document.createElement("section");
    container.id = "notificationStack";
    container.className = "notification-stack";
    container.setAttribute("aria-label", "");
    container.dataset.translateAriaLabel = "desktop.notifications";
    void t("desktop.notifications").then((copy) => container.setAttribute("aria-label", copy));
    document.body.appendChild(container);
  }
  return container;
}

function markRead(id) {
  if (!id) return;
  try { localStorage.setItem(storageKey, JSON.stringify([...new Set([...readNews(), id])].slice(-200))); }
  catch (error) { console.warn("[notifications] news preferences unavailable", error); }
}

function arm(entry) {
  clearTimeout(entry.timer);
  if (entry.duration > 0) entry.timer = setTimeout(() => dismiss(entry.key), entry.remaining);
  entry.started = Date.now();
}

function drain() {
  while (!disposed && visible.length < 4 && queue.length) {
    const entry = queue.shift();
    visible.push(entry);
    stack().appendChild(entry.node);
    arm(entry);
  }
}

export function dismiss(key) {
  const entry = entries.get(key);
  if (!entry) return;
  clearTimeout(entry.timer);
  entry.node.remove();
  entries.delete(key);
  for (const list of [queue, visible]) {
    const index = list.indexOf(entry);
    if (index !== -1) list.splice(index, 1);
  }
  markRead(entry.newsId);
  drain();
}

export async function notify(message, options = {}) {
  if (disposed || (options.newsId && readNews().includes(options.newsId))) return null;
  const type = ["info", "success", "warning", "error"].includes(options.type) ? options.type : "info";
  const key = options.id || `${type}:${message}`;
  const existing = entries.get(key);
  if (existing) {
    existing.count++;
    existing.countNode.textContent = `×${existing.count}`;
    existing.countNode.hidden = false;
    // Repeated errors remain one item; repetition does not postpone dismissal.
    return key;
  }
  const node = document.createElement("article");
  node.className = `desktop-notification toast-${type} career-toast`;
  node.dataset.type = type;
  node.setAttribute("role", type === "error" ? "alert" : "status");
  node.setAttribute("aria-atomic", "true");
  const copy = document.createElement("div");
  copy.className = "notification-copy";
  const text = document.createElement("span");
  text.textContent = message;
  const countNode = document.createElement("small");
  countNode.hidden = true;
  countNode.className = "notification-count";
  copy.append(text, countNode);
  const close = document.createElement("button");
  close.type = "button";
  close.className = "notification-close";
  close.textContent = "×";
  close.dataset.translateAriaLabel = "ui.aria_close";
  close.addEventListener("click", () => dismiss(key));
  node.append(copy, close);
  const duration = options.critical || type === "error" ? 0 : Math.max(0, options.duration ?? 6000);
  const entry = { key, node, countNode, count: 1, duration, remaining: duration, newsId: options.newsId };
  entries.set(key, entry);
  queue.push(entry);
  // Hover and keyboard interaction pause the remaining display duration.
  const pause = () => {
    if (!entry.timer) return;
    clearTimeout(entry.timer);
    entry.timer = null;
    entry.remaining = Math.max(1, entry.remaining - (Date.now() - entry.started));
  };
  const resume = () => { if (!node.matches(":hover, :focus-within")) arm(entry); };
  node.addEventListener("mouseenter", pause);
  node.addEventListener("mouseleave", resume);
  node.addEventListener("focusin", pause);
  node.addEventListener("focusout", () => queueMicrotask(resume));
  drain();
  close.setAttribute("aria-label", await t("ui.aria_close"));
  if (options.details) {
    const details = document.createElement("details");
    const summary = document.createElement("summary");
    summary.textContent = await t("desktop.technical_details");
    const pre = document.createElement("pre");
    pre.tabIndex = 0;
    pre.textContent = String(options.details);
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = await t("desktop.copy_details");
    button.addEventListener("click", async () => {
      try { await navigator.clipboard.writeText(pre.textContent); button.textContent = await t("desktop.copied"); }
      catch { pre.focus(); const range = document.createRange(); range.selectNodeContents(pre); getSelection().removeAllRanges(); getSelection().addRange(range); }
    });
    details.append(summary, pre, button);
    copy.append(details);
  }
  return key;
}

export const notifications = {
  notify, dismiss,
  get size() { return visible.length; },
  get queued() { return queue.length; },
  clear() { for (const key of [...entries.keys()]) dismiss(key); },
};
export async function showToast(messageOrKey, options = {}, type = "info") {
  const params = typeof options === "string" ? {} : options;
  return notify(await t(messageOrKey, params), { ...params, type: typeof options === "string" ? options : type });
}
window.notificationManager = notifications;
window.showToast ||= showToast;
window.addEventListener("pagehide", () => {
  disposed = true;
  for (const entry of entries.values()) clearTimeout(entry.timer);
});
window.addEventListener("pageshow", () => {
  disposed = false;
  visible.forEach(arm);
  drain();
});
