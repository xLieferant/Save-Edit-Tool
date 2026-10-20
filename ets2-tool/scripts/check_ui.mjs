// Read-only browser fixture checks. This does not exercise native Tauri commands.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import http from "node:http";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const root = fileURLToPath(new URL("../", import.meta.url));
const browser = process.env.CHROME_PATH || "C:/Program Files/Google/Chrome/Application/chrome.exe";
if (!fs.existsSync(browser)) throw new Error("Set CHROME_PATH to a local Chromium executable.");
const artifacts = fs.mkdtempSync(path.join(os.tmpdir(), "ets2-ui-review-"));
const sourceRoot = path.join(root, "src");
const server = http.createServer((request, response) => {
  const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
  const file = path.resolve(sourceRoot, `.${pathname === "/" ? "/index.html" : pathname}`);
  if (!file.startsWith(`${sourceRoot}${path.sep}`) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
    response.writeHead(404).end();
    return;
  }
  const types = { ".js": "text/javascript", ".css": "text/css", ".html": "text/html", ".json": "application/json", ".png": "image/png", ".jpg": "image/jpeg" };
  response.setHeader("Content-Type", types[path.extname(file)] || "application/octet-stream");
  response.end(fs.readFileSync(file));
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const child = spawn(browser, ["--headless", "--disable-gpu", "--no-first-run", "--disable-background-networking",
  "--disable-extensions", "--remote-debugging-port=0", `--user-data-dir=${path.join(artifacts, "profile")}`, "about:blank"],
{ windowsHide: true, stdio: ["ignore", "ignore", "pipe"] });
let socket;
try {
  const endpoint = await new Promise((resolve, reject) => {
    let output = "";
    const timeout = setTimeout(() => reject(new Error("Browser did not start within 20 seconds.")), 20000);
    child.on("error", reject);
    child.stderr.on("data", (chunk) => {
      output += chunk;
      const match = output.match(/DevTools listening on (ws:\/\/\S+)/);
      if (match) { clearTimeout(timeout); resolve(match[1]); }
    });
  });
  const debugUrl = new URL(endpoint);
  const targets = await (await fetch(`http://${debugUrl.host}/json/list`)).json();
  socket = new WebSocket(targets.find((target) => target.type === "page").webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let id = 0;
  const pending = new Map();
  const exceptions = [];
  socket.onmessage = ({ data }) => {
    const message = JSON.parse(data);
    if (message.method === "Runtime.exceptionThrown") exceptions.push(message.params.exceptionDetails.exception?.description || message.params.exceptionDetails.text);
    if (pending.has(message.id)) {
      const { resolve, reject, timer } = pending.get(message.id);
      clearTimeout(timer);
      pending.delete(message.id);
      if (message.error) reject(new Error(JSON.stringify(message.error)));
      else resolve(message.result);
    }
  };
  const call = (method, params = {}) => new Promise((resolve, reject) => {
    const requestId = ++id;
    const timer = setTimeout(() => { pending.delete(requestId); reject(new Error(`CDP timeout: ${method}`)); }, 15000);
    pending.set(requestId, { resolve, reject, timer });
    socket.send(JSON.stringify({ id: requestId, method, params }));
  });
  const evaluate = async (expression) => {
    const result = await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  await call("Page.enable");
  await call("Runtime.enable");
  const locales = Object.fromEntries(["en", "de"].map((lang) => [lang, JSON.parse(fs.readFileSync(path.join(root, `src-tauri/locales/${lang}.json`), "utf8"))]));
  const pages = ["/index.html", ...fs.readdirSync(path.join(sourceRoot, "pages")).map((name) => `/pages/${name}/index.html`)];
  let checks = 0;
  for (const language of ["de", "en"]) {
    const { identifier } = await call("Page.addScriptToEvaluateOnNewDocument", { source: `
      window.__fixtureLanguage = ${JSON.stringify(language)};
      const locales = ${JSON.stringify(locales)};
      const lookup = (lang, key) => key.split(".").reduce((value, part) => value?.[part], locales[lang]);
      window.__TAURI__ = {
        core: { convertFileSrc: (path) => path, invoke: async (command, args = {}) => {
          if (command === "translate_command") return lookup(window.__fixtureLanguage, args.key) ?? lookup("en", args.key) ?? args.key;
          if (command === "get_current_language_command") return window.__fixtureLanguage;
          if (command === "get_available_languages_command") return [{ code: "de", name: "Deutsch" }, { code: "en", name: "English" }];
          if (command === "set_language_command") { window.__fixtureLanguage = args.language; return ""; }
          if (command === "hub_get_mode") return "editor";
          if (command === "get_selected_game") return "ets2";
          if (["dispatcher_get_market_jobs", "dispatcher_get_active_jobs", "dispatcher_get_company_contacts", "dispatcher_get_offers", "get_company_members", "career_get_job_log"].includes(command)) return [];
          if (/^(find_|list_|scan_|get_available_|get_user_logs|get_user_sessions)/.test(command)) return [];
          return null;
        } },
        event: { listen: async () => () => {} },
        app: { getVersion: async () => "0.9.8" },
        opener: { openUrl: async () => {} },
      };
      localStorage.setItem("theme", "dark");
    ` });
    for (const page of pages) {
      for (const [width, height] of [[900, 640], [1210, 800], [1600, 1000]]) {
        exceptions.length = 0;
        await call("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
        await call("Page.navigate", { url: `http://127.0.0.1:${server.address().port}${page}` });
        await evaluate(`new Promise(resolve => { const check = () => document.readyState === "complete" ? setTimeout(resolve, 350) : setTimeout(check, 25); check(); })`);
        const result = await evaluate(`(async () => {
          await window.translateUI?.();
          const missing = [...document.querySelectorAll("[data-translate]")].filter(node => node.textContent === node.dataset.translate).map(node => node.dataset.translate);
          const invalidStyles = [...document.styleSheets].flatMap(sheet => { try { return [...sheet.cssRules].length ? [] : [sheet.href]; } catch { return [sheet.href]; } });
          const rgb = value => (value.match(/[\\d.]+/g) || []).map(Number);
          const light = color => color.slice(0, 3).map(value => value / 255).map(value => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4).reduce((sum, value, index) => sum + value * [.2126, .7152, .0722][index], 0);
          const backgroundColor = node => {
            const parents = [];
            for (let parent = node; parent; parent = parent.parentElement) parents.unshift(parent);
            return parents.reduce((surface, parent) => {
              const color = rgb(getComputedStyle(parent).backgroundColor);
              const alpha = color[3] ?? 1;
              return surface.map((value, index) => color[index] * alpha + value * (1 - alpha));
            }, [23, 25, 28]);
          };
          const lowContrast = [...document.querySelectorAll('button:not(:disabled)')].filter(node => {
            const style = getComputedStyle(node);
            if (!node.getClientRects().length || style.visibility === 'hidden' || Number(style.opacity) < 1 || node.getAttribute('aria-disabled') === 'true' || !node.textContent.trim()) return false;
            const color = light(rgb(style.color));
            const surface = light(backgroundColor(node));
            return (Math.max(color, surface) + .05) / (Math.min(color, surface) + .05) < 4.5;
          }).map(node => node.id || node.textContent.trim());
          const visibleHidden = [...document.querySelectorAll('[hidden]')].filter(node => getComputedStyle(node).display !== 'none').map(node => node.id || node.tagName);
          return { lang: document.documentElement.lang, missing, lowContrast, visibleHidden, accent: getComputedStyle(document.body).getPropertyValue('--accent').trim(), untranslated: document.body.textContent.includes('[NEEDS TRANSLATION]'), overflow: document.documentElement.scrollWidth > innerWidth + 1, invalidStyles, design: !!document.querySelector('link[href="/styles/design_system.css"]') };
        })()`);
        assert.equal(result.lang, language, `${page}: document language`);
        assert.deepEqual(result.missing, [], `${page}: missing translations`);
        assert.equal(result.untranslated, false, `${page}: untranslated markers`);
        assert.equal(result.accent, "#8fb8cd", `${page}: shared dark theme tokens`);
        assert.deepEqual(result.lowContrast, [], `${page}: button text contrast below 4.5:1`);
        assert.deepEqual(result.visibleHidden, [], `${page}: hidden states are visible`);
        assert.deepEqual(result.invalidStyles, [], `${page}: unreadable stylesheet`);
        assert.equal(result.design, true, `${page}: central design stylesheet`);
        assert.equal(result.overflow, false, `${page} ${width}: horizontal page overflow`);
        assert.deepEqual(exceptions, [], `${page}: uncaught browser errors`);
        if (page.includes("save-editor") && width === 1210) {
          for (const tab of ["truck", "trailer", "profile", "garages", "settings"]) {
            await evaluate(`document.querySelector('.editor-tabs [data-tab="${tab}"]')?.click(); new Promise(resolve => setTimeout(resolve, 150))`);
            assert.equal(await evaluate(`document.querySelector('.editor-tabs [data-tab="${tab}"]').classList.contains("active")`), true);
          }
        }
        if (width === 1210) {
          const otherLanguage = language === "de" ? "en" : "de";
          const switched = await evaluate(`(async () => {
            await window.__TAURI__.core.invoke("set_language_command", { language: ${JSON.stringify(otherLanguage)} });
            await window.translateUI?.();
            const translated = await window.t("ui.export_path", { path: "/fixture/export.csv" });
            return { language: document.documentElement.lang, translated };
          })()`);
          assert.equal(switched.language, otherLanguage, `${page}: language switch`);
          assert.equal(switched.translated, locales[otherLanguage].ui.export_path.replaceAll("{path}", "/fixture/export.csv"));
          await evaluate(`(async () => { await window.__TAURI__.core.invoke("set_language_command", { language: ${JSON.stringify(language)} }); await window.translateUI?.(); })()`);
          const screenshot = await call("Page.captureScreenshot", { format: "png" });
          fs.writeFileSync(path.join(artifacts, `${language}-${page.split("/").at(-2) || "launcher"}.png`), Buffer.from(screenshot.data, "base64"));
        }
        assert.deepEqual(exceptions, [], `${page}: uncaught errors after navigation and translation`);
        checks++;
      }
    }
    await call("Page.removeScriptToEvaluateOnNewDocument", { identifier });
  }
  console.log(`${checks} page/language/window fixture checks passed; editor tabs checked in both languages.`);
  console.log(`Screenshots: ${artifacts}`);
  console.log("Native file dialogs, profile scanning, save writes and real game data are not covered by this fixture.");
} finally {
  socket?.close();
  child.kill();
  server.close();
}
