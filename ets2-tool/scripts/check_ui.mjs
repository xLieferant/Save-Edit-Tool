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
  let appearanceChecks = 0;
  let garageChecks = 0;
  const delay = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));
  const waitFor = async (expression) => {
    for (let attempt = 0; attempt < 100; attempt++) {
      try { if (await evaluate(expression)) return; } catch (error) {
        if (!/context|navigat|closed/i.test(error.message)) throw error;
      }
      await delay(30);
    }
    throw new Error(`Browser condition timed out: ${expression}`);
  };
  const navigate = async (page) => {
    await call("Page.navigate", { url: `http://127.0.0.1:${server.address().port}${page}` });
    await waitFor(`location.pathname === ${JSON.stringify(page.split("?")[0])} && document.readyState === "complete"`);
    await delay(150);
  };
  const click = async (selector) => {
    const point = await evaluate(`(() => { const node = document.querySelector(${JSON.stringify(selector)}); node.scrollIntoView({ block: "center" }); const r = node.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
    await call("Input.dispatchMouseEvent", { type: "mousePressed", button: "left", clickCount: 1, ...point });
    await call("Input.dispatchMouseEvent", { type: "mouseReleased", button: "left", clickCount: 1, ...point });
  };
  const space = async () => {
    await call("Input.dispatchKeyEvent", { type: "keyDown", key: " ", code: "Space", windowsVirtualKeyCode: 32 });
    await call("Input.dispatchKeyEvent", { type: "keyUp", key: " ", code: "Space", windowsVirtualKeyCode: 32 });
  };
  for (const language of ["de", "en"]) {
    const { identifier } = await call("Page.addScriptToEvaluateOnNewDocument", { source: `
      window.__fixtureLanguage = ${JSON.stringify(language)};
      const locales = ${JSON.stringify(locales)};
      const lookup = (lang, key) => key.split(".").reduce((value, part) => value?.[part], locales[lang]);
      window.__fixtureGarageCount = 0;
      window.__fixtureCalls = [];
      window.__fixtureCareerSettings = { telemetryEnabled: false };
      window.__fixtureListeners = {};
      const garages = () => ({
        game: "ets2", saveHash: "fixture-hash", headquartersGarageId: null,
        diagnostics: { warnings: [] },
        garages: Array.from({ length: window.__fixtureGarageCount }, (_, index) => ({
          garageId: "garage.fixture_" + index, cityName: "Fixture city " + String(index).padStart(3, "0"),
          cityToken: "fixture_" + index, countryCode: "DE", size: "small", ownership: "owned",
          vehicleSlotCount: 3, driverSlotCount: 3, trailerSlotCount: 0, maximumSlotCount: 5,
          occupiedSlots: 0, availableSlots: 3, assignedDriverCount: 0, assignedTruckCount: 0,
          assignedTrailerCount: 0, slots: [], trailerIds: [], isHeadquarters: false,
          capacityConsistent: true, warnings: [],
        })),
      });
      window.__TAURI__ = {
        core: { convertFileSrc: (path) => path, invoke: async (command, args = {}) => {
          window.__fixtureCalls.push(command);
          if (window.__fixtureDelay?.[command]) await new Promise(resolve => setTimeout(resolve, window.__fixtureDelay[command]));
          if (window.__fixtureResponses && Object.hasOwn(window.__fixtureResponses, command)) return structuredClone(window.__fixtureResponses[command]);
          if (command === window.__fixtureFailCommand) {
            await new Promise(resolve => setTimeout(resolve, 180));
            throw new Error("not_allowed");
          }
          if (command === "translate_command") return lookup(window.__fixtureLanguage, args.key) ?? lookup("en", args.key) ?? args.key;
          if (command === "get_current_language_command") return window.__fixtureLanguage;
          if (command === "get_available_languages_command") return [{ code: "de", name: "Deutsch" }, { code: "en", name: "English" }];
          if (command === "set_language_command") { window.__fixtureLanguage = args.language; return ""; }
          if (command === "hub_get_mode") return localStorage.getItem("__fixtureBackendMode") || "editor";
          if (command === "hub_set_mode") { localStorage.setItem("__fixtureBackendMode", args.mode); return args.mode; }
          if (command === "get_all_garages") return garages();
          if (command === "get_ai_driver_management") return { game: "ets2", saveHash: "fixture-hash", drivers: [], availableDriverCount: 10, availableTrucks: [], warnings: [] };
          if (command === "get_career_settings") return window.__fixtureCareerSettings;
          if (command === "update_career_settings") { window.__fixtureCareerSettings = args.input; return args.input; }
          if (command === "get_selected_game") return "ets2";
          if (["dispatcher_get_market_jobs", "dispatcher_get_active_jobs", "dispatcher_get_company_contacts", "dispatcher_get_offers", "get_company_members", "career_get_job_log"].includes(command)) return [];
          if (/^(find_|list_|scan_|get_available_|get_user_logs|get_user_sessions)/.test(command)) return [];
          return null;
        } },
        event: { listen: async (name, callback) => { window.__fixtureListeners[name] = callback; return () => { delete window.__fixtureListeners[name]; }; } },
        app: { getVersion: async () => "0.9.8" },
        opener: { openUrl: async () => {} },
      };
      if (!localStorage.getItem("theme")) localStorage.setItem("theme", "dark");
    ` });
    for (const page of process.argv.includes("--refinement-only") ? [] : pages) {
      for (const [width, height] of [[900, 640], [1210, 800], [1600, 1000]]) {
        exceptions.length = 0;
        await call("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
        await call("Page.navigate", { url: `http://127.0.0.1:${server.address().port}${page}${page === "/index.html" ? "?choose=1" : ""}` });
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
          window.__lowContrastButtons = () => [...document.querySelectorAll('button:not(:disabled)')].filter(node => {
            const style = getComputedStyle(node);
            if (!node.getClientRects().length || style.visibility === 'hidden' || Number(style.opacity) < 1 || node.getAttribute('aria-disabled') === 'true' || !node.textContent.trim()) return false;
            const color = light(rgb(style.color));
            const surface = light(backgroundColor(node));
            return (Math.max(color, surface) + .05) / (Math.min(color, surface) + .05) < 4.5;
          }).map(node => node.id || node.textContent.trim());
          const lowContrast = window.__lowContrastButtons();
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
          for (const theme of ["dark", "light"]) {
            for (const accent of ["default", "green"]) {
              const appearance = await evaluate(`(() => {
                window.themePreferences.apply({ theme: ${JSON.stringify(theme)}, accent: ${JSON.stringify(accent)} });
                const style = getComputedStyle(document.body);
                const credit = document.querySelector('.creator-credit');
                return { theme: document.documentElement.dataset.theme, accent: document.documentElement.dataset.accent,
                  background: style.backgroundColor, text: style.color,
                  storedTheme: localStorage.getItem('theme'), storedAccent: localStorage.getItem('accent'),
                  credit: credit?.textContent, creditHeight: credit?.getBoundingClientRect().height,
                  lowContrast: window.__lowContrastButtons() };
              })()`);
              assert.equal(appearance.theme, theme);
              assert.equal(appearance.accent, accent);
              assert.equal(appearance.storedTheme, theme);
              assert.equal(appearance.storedAccent, accent);
              assert.deepEqual(appearance.lowContrast, [], `${page}: ${theme}/${accent} button contrast`);
              assert.equal(appearance.background, theme === "dark" ? "rgb(23, 25, 28)" : "rgb(238, 240, 242)");
              assert.equal(appearance.text, theme === "dark" ? "rgb(230, 232, 235)" : "rgb(36, 41, 48)");
              if (page.includes("save-editor") || page.includes("/career/")) {
                assert.equal(appearance.credit, locales[language].ui_refinement.creator_credit);
                assert.ok(appearance.creditHeight > 0, `${page}: creator credit is visible`);
                const shot = await call("Page.captureScreenshot", { format: "png" });
                fs.writeFileSync(path.join(artifacts, `${language}-${page.split("/").at(-2)}-${theme}-${accent}.png`), Buffer.from(shot.data, "base64"));
              }
              appearanceChecks++;
            }
          }
          await evaluate('window.themePreferences.apply({ theme: "dark", accent: "default" })');
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

    // Shared native checkbox/switch behavior, including keyboard and disabled states.
    await navigate("/pages/save-editor/index.html");
    await evaluate('(async () => { const editor = await import("/app.js"); window.__switchResult = editor.openModalSlider("ui_refinement.accent_label", "0"); })()');
    await waitFor('getComputedStyle(document.getElementById("modalSlider")).display !== "none"');
    assert.equal(await evaluate('document.getElementById("modalSliderInput").checked'), false);
    const switchedLanguage = language === "de" ? "en" : "de";
    await evaluate(`(async () => { await window.__TAURI__.core.invoke("set_language_command", { language: ${JSON.stringify(switchedLanguage)} }); await window.translateUI(); })()`);
    assert.equal(await evaluate('document.querySelector("#modalSlider .toggle-switch__state").textContent'), locales[switchedLanguage].label.toggle_off);
    await evaluate('document.getElementById("modalSliderInput").focus()');
    await space();
    await waitFor('Math.abs(new DOMMatrix(getComputedStyle(document.querySelector("#modalSlider .toggle-switch__slider"), "::before").transform).m41 - 20) < 0.1');
    assert.equal(await evaluate('document.getElementById("modalSliderInput").checked'), true);
    assert.equal(await evaluate('document.getElementById("modalSliderInput").getAttribute("aria-checked")'), "true");
    assert.equal(await evaluate('document.querySelector("#modalSlider .toggle-switch__state").textContent'), locales[switchedLanguage].label.toggle_on);
    await evaluate(`(async () => { await window.__TAURI__.core.invoke("set_language_command", { language: ${JSON.stringify(language)} }); await window.translateUI(); })()`);
    assert.ok(Math.abs(await evaluate('new DOMMatrix(getComputedStyle(document.querySelector("#modalSlider .toggle-switch__slider"), "::before").transform).m41') - 20) < 0.1);
    await click("#modalSlider .toggle-switch__slider");
    await waitFor('Math.abs(new DOMMatrix(getComputedStyle(document.querySelector("#modalSlider .toggle-switch__slider"), "::before").transform).m41) < 0.1');
    assert.equal(await evaluate('document.getElementById("modalSliderInput").checked'), false);
    assert.ok(Math.abs(await evaluate('new DOMMatrix(getComputedStyle(document.querySelector("#modalSlider .toggle-switch__slider"), "::before").transform).m41')) < 0.1);
    await evaluate('document.getElementById("modalSliderInput").disabled = true');
    await click("#modalSlider .toggle-switch__slider");
    assert.equal(await evaluate('document.getElementById("modalSliderInput").checked'), false);
    await evaluate('document.getElementById("modalSliderInput").disabled = false; document.getElementById("modalSliderApply").click()');
    assert.equal(await evaluate('window.__switchResult'), 0);

    await click("#saveColorSchemeBtn");
    await waitFor('getComputedStyle(document.getElementById("modalMulti")).display !== "none"');
    assert.deepEqual(await evaluate('[...document.querySelectorAll("#modalMulti #theme option")].map(option => option.value)'), ["dark", "light"]);
    assert.deepEqual(await evaluate('[...document.querySelectorAll("#modalMulti #accent option")].map(option => option.value)'), ["default", "green"]);
    await evaluate('document.querySelector("#modalMulti #theme").value = "light"; document.querySelector("#modalMulti #accent").value = "green"; document.getElementById("modalMultiApply").click()');
    await waitFor('document.documentElement.dataset.theme === "light" && document.documentElement.dataset.accent === "green"');
    await navigate("/pages/save-editor/index.html");
    assert.deepEqual(await evaluate('window.themePreferences.get()'), { theme: "light", accent: "green" });
    await click("#saveColorSchemeBtn");
    await waitFor('getComputedStyle(document.getElementById("modalMulti")).display !== "none"');
    await evaluate('document.querySelector("#modalMulti #theme").value = "dark"; document.querySelector("#modalMulti #accent").value = "default"; document.getElementById("modalMultiCancel").click()');
    assert.deepEqual(await evaluate('window.themePreferences.get()'), { theme: "light", accent: "green" }, "cancel must preserve appearance");

    // Real garage renderer, with isolated read-only command fixtures.
    for (const [width, height] of [[900, 640], [1210, 800]]) {
      await call("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
      for (const theme of ["dark", "light"]) {
        for (const accent of ["default", "green"]) {
          for (const count of [0, 1, 10, 50, 100, 125]) {
            const layout = await evaluate(`(async () => {
              window.themePreferences.apply({ theme: ${JSON.stringify(theme)}, accent: ${JSON.stringify(accent)} });
              window.selectedProfilePath = '/fixture/profile'; window.selectedSavePath = '/fixture/profile/save';
              window.__fixtureGarageCount = ${count};
              const root = document.getElementById('tool-container'); root.innerHTML = ''; root.dataset.view = 'garages';
              const scroll = document.querySelector('.save-content-scroll'); scroll.scrollTop = 0;
              const { mountGarageManager } = await import('/js/garage-manager.js');
              await mountGarageManager(root);
              // Let the manager finish restoring the previous view position.
              await new Promise(resolve => requestAnimationFrame(resolve));
              scroll.scrollTop = scroll.scrollHeight;
              await new Promise(resolve => requestAnimationFrame(resolve));
              const list = root.querySelector('.garage-list');
              const cards = [...list.querySelectorAll('.garage-card')];
              const owners = [];
              for (let parent = list; parent; parent = parent.parentElement) {
                const overflow = getComputedStyle(parent).overflowY;
                if (['auto', 'scroll'].includes(overflow) && parent.scrollHeight > parent.clientHeight + 2) owners.push(parent.className);
              }
              const footer = document.querySelector('.editor-sidebar > .creator-credit').getBoundingClientRect();
              const sidebar = document.querySelector('.editor-sidebar-scroll');
              const scan = document.getElementById('refreshBtn');
              const scanRect = scan.getBoundingClientRect();
              const scanBlock = scan.closest('.editor-sidebar-block').getBoundingClientRect();
              sidebar.scrollTop = sidebar.scrollHeight;
              const lastAction = [...sidebar.querySelectorAll('button')].filter(node => node.getBoundingClientRect().height > 0).at(-1);
              const actionRect = lastAction.getBoundingClientRect();
              const sidebarRect = sidebar.getBoundingClientRect();
              return { count: cards.length, owners, scrollTop: scroll.scrollTop,
                lastBottom: cards.at(-1)?.getBoundingClientRect().bottom || 0, scrollBottom: scroll.getBoundingClientRect().bottom,
                pageOverflow: document.documentElement.scrollWidth > innerWidth + 1,
                footerVisible: footer.top >= 0 && footer.bottom <= innerHeight + 1,
                scanUnclipped: scanRect.top >= scanBlock.top && scanRect.bottom <= scanBlock.bottom + 1,
                sidebarActionsReachable: actionRect.top >= sidebarRect.top && actionRect.bottom <= sidebarRect.bottom + 1,
                clippedControls: [...root.querySelectorAll('input, select, button')].filter(node => {
                  const r = node.getBoundingClientRect(); const area = scroll.getBoundingClientRect();
                  return r.width > 0 && (r.right > area.right + 1 || r.left < area.left - 1);
                }).map(node => node.id || node.textContent),
                outerBorder: getComputedStyle(root).borderTopWidth,
                headerBorder: getComputedStyle(root.querySelector('.garage-manager-head')).borderTopWidth };
            })()`);
            assert.equal(layout.count, count, `garages ${count} ${width} ${theme}/${accent}: rendered count`);
            assert.equal(layout.pageOverflow, false, `garages ${count} ${width}: page overflow`);
            assert.deepEqual(layout.clippedControls, [], `garages ${count} ${width}: clipped controls`);
            assert.equal(layout.footerVisible, true, `garages ${count} ${width}: sidebar credit`);
            assert.equal(layout.scanUnclipped, true, `garages ${count} ${width}: profile scan action is not clipped`);
            assert.equal(layout.sidebarActionsReachable, true, `garages ${count} ${width}: last sidebar action remains reachable`);
            assert.equal(layout.outerBorder, "0px");
            assert.equal(layout.headerBorder, "0px");
            assert.ok(layout.owners.length <= 1, `garages ${count} ${width}: nested vertical scroll areas: ${layout.owners}`);
            if (count >= 10) {
              assert.equal(layout.owners.length, 1, "one active data scroll area");
              assert.ok(layout.scrollTop > 0, `large garage view scrolls: ${JSON.stringify({ width, height, theme, accent, count, ...layout })}`);
              assert.ok(layout.lastBottom <= layout.scrollBottom + 2, "last garage remains reachable");
            }
            garageChecks++;
          }
        }
      }
    }
    const garageShot = await call("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(path.join(artifacts, `${language}-garages-125-light-green.png`), Buffer.from(garageShot.data, "base64"));
    await evaluate('document.querySelector("[data-ai-drivers]").click()');
    await waitFor('Boolean(document.querySelector("[data-ai-driver-assign-trucks]"))');
    await click("[data-ai-driver-assign-trucks]");
    assert.equal(await evaluate('document.querySelector("[data-ai-driver-assign-trucks]").checked'), true);
    assert.notEqual(await evaluate('getComputedStyle(document.querySelector("[data-ai-driver-assign-trucks]")).backgroundImage'), "none");
    await evaluate('document.querySelector("[data-ai-driver-assign-trucks]").focus()');
    await space();
    assert.equal(await evaluate('document.querySelector("[data-ai-driver-assign-trucks]").checked'), false);
    await evaluate('document.querySelector("[data-ai-driver-assign-trucks]").disabled = true');
    await click("[data-ai-driver-assign-trucks]");
    assert.equal(await evaluate('document.querySelector("[data-ai-driver-assign-trucks]").checked'), false);
    assert.equal(await evaluate('window.__fixtureCalls.some(command => /^(purchase_garage|update_garage|generate_ai|remove_ai)/.test(command))'), false);

    // Failed writes reset the switch; background updates preserve unsaved edits.
    await navigate("/pages/career/index.html");
    await waitFor('window.__fixtureCalls.includes("get_career_settings")');
    await evaluate('document.querySelector(".career-nav-btn[data-panel=settings]").click()');
    assert.equal(await evaluate('document.getElementById("cmCareerTelemetryEnabledInput").checked'), false);
    await click("#cmCareerTelemetryEnabledInput + .toggle-switch__slider");
    assert.equal(await evaluate('document.getElementById("cmCareerTelemetryEnabledInput").checked'), true);
    await evaluate('window.__fixtureListeners["career://overview"]?.({ payload: {} })');
    assert.equal(await evaluate('document.getElementById("cmCareerTelemetryEnabledInput").checked'), true, "background refresh must preserve pending input");
    await evaluate('window.__fixtureFailCommand = "update_career_settings"; document.getElementById("cmSaveCareerSettingsBtn").click()');
    await waitFor('document.getElementById("cmSaveCareerSettingsBtn").disabled');
    assert.equal(await evaluate('document.getElementById("cmCareerTelemetryEnabledInput").disabled'), true);
    await waitFor('!document.getElementById("cmSaveCareerSettingsBtn").disabled');
    assert.equal(await evaluate('document.getElementById("cmCareerTelemetryEnabledInput").checked'), false);
    assert.equal(await evaluate('document.getElementById("cmCareerTelemetryEnabledInput").getAttribute("aria-checked")'), "false");
    assert.ok(await evaluate('document.querySelector(".career-toast[data-type=error]")?.textContent'));
    await evaluate('window.__fixtureFailCommand = null');
    await click("#cmCareerTelemetryEnabledInput + .toggle-switch__slider");
    await evaluate('document.getElementById("cmSaveCareerSettingsBtn").click()');
    await waitFor('!document.getElementById("cmSaveCareerSettingsBtn").disabled');
    assert.equal(await evaluate('window.__fixtureCareerSettings.telemetryEnabled'), true);
    assert.equal(await evaluate('document.getElementById("cmCareerTelemetryEnabledInput").dataset.dirty'), undefined);

    // Shared notifications: mixed heights, FIFO queue, deduplication and read news.
    await evaluate(`(async () => {
      window.notificationManager.clear();
      await window.notificationManager.notify('Persistent fixture error', {type:'error',id:'critical'});
      for (let index = 1; index <= 5; index++) await window.notificationManager.notify('Fixture ' + index + ' — '.repeat(index * 30), {id:'fixture-' + index,duration:0});
    })()`);
    assert.deepEqual(await evaluate('({visible:window.notificationManager.size,queued:window.notificationManager.queued})'), { visible: 4, queued: 2 });
    await evaluate("window.notificationManager.notify('Persistent fixture error', {type:'error',id:'critical'})");
    assert.equal(await evaluate('document.querySelector(".notification-count").textContent'), '×2');
    for (const scale of [1, 1.25, 1.5, 2]) {
      await call("Emulation.setDeviceMetricsOverride", { width: 900, height: 640, deviceScaleFactor: scale, mobile: false });
      const stack = await evaluate(`(() => {
        const boxes = [...document.querySelectorAll('.desktop-notification')].map(node => node.getBoundingClientRect());
        return { overlap:boxes.some((box,index) => index && boxes[index-1].bottom > box.top), overflow:boxes.some(box => box.right > innerWidth || box.left < 0), order:[...document.querySelectorAll('.notification-copy > span')].map(node => node.textContent) };
      })()`);
      assert.equal(stack.overlap, false);
      assert.equal(stack.overflow, false);
      assert.ok(stack.order[0].startsWith('Persistent'));
      assert.ok(stack.order.at(-1).startsWith('Fixture 3'));
    }
    await evaluate('document.querySelector(".notification-close").click()');
    assert.deepEqual(await evaluate('({visible:window.notificationManager.size,queued:window.notificationManager.queued})'), { visible: 4, queued: 1 });
    assert.ok(await evaluate('[...document.querySelectorAll(".notification-copy > span")].at(-1).textContent.startsWith("Fixture 4")'));
    await evaluate(`(async () => {
      window.notificationManager.clear();
      localStorage.removeItem('ets2_read_news_v1');
      const key = await window.notificationManager.notify('News fixture', {newsId:'news-regression',duration:0});
      window.notificationManager.dismiss(key);
      await window.notificationManager.notify('News fixture', {newsId:'news-regression'});
    })()`);
    assert.equal(await evaluate('window.notificationManager.size'), 0, "read news must stay dismissed");
    await evaluate("window.notificationManager.notify('Short fixture', {duration:80})");
    await waitFor('window.notificationManager.size === 0');

    // Career refresh reads data without replacing the document or losing a draft.
    await evaluate(`(() => {
      window.__documentToken = 'retained';
      const input = document.getElementById('cmUserUsernameInput');
      input.value = 'Unsaved draft'; input.dispatchEvent(new Event('input',{bubbles:true}));
      window.__fixtureCalls = [];
      window.__fixtureDelay = {career_get_status:120};
    })()`);
    await evaluate('Promise.all([window.frontendState.refresh(),window.frontendState.refresh(),window.frontendState.refresh()])');
    assert.equal(await evaluate('window.__fixtureCalls.filter(command => command === "career_get_status").length'), 1, "concurrent refreshes share reads");
    await click('#refreshCareerBtn');
    await waitFor('!document.getElementById("refreshCareerBtn").disabled');
    assert.equal(await evaluate('window.__documentToken'), 'retained');
    assert.equal(await evaluate('document.getElementById("cmUserUsernameInput").value'), 'Unsaved draft');
    await evaluate('window.__fixtureDelay = {}');

    // Search and sorting remain attached when a management table gets new rows.
    await evaluate(`localStorage.removeItem('desktop:table:/pages/career/index.html:regressionTable'); document.querySelector('.career-panel.is-active').insertAdjacentHTML('beforeend','<div id="tableFixture"><table id="regressionTable"><thead><tr><th>Name</th><th>Value</th></tr></thead><tbody><tr><td>Beta</td><td>10</td></tr><tr><td>Alpha</td><td>2</td></tr></tbody></table></div>')`);
    await waitFor('Boolean(document.querySelector("#tableFixture input[type=search]"))');
    await evaluate('document.querySelector("#regressionTable th button").click()');
    assert.equal(await evaluate('document.querySelector("#regressionTable tbody tr td").textContent'), 'Alpha');
    await evaluate(`(() => { const input = document.querySelector('#tableFixture input'); input.value = 'Beta'; input.dispatchEvent(new Event('input')); })()`);
    await waitFor('document.querySelectorAll("#regressionTable tbody tr:not([hidden])").length === 1');
    assert.equal(await evaluate('document.querySelector("#regressionTable tbody tr:not([hidden]) td").textContent'), 'Beta');
    await evaluate('document.querySelector("#regressionTable tbody").innerHTML = "<tr><td>Gamma</td><td>1</td></tr><tr><td>Beta</td><td>3</td></tr>"');
    await waitFor('document.querySelectorAll("#regressionTable tbody tr:not([hidden])").length === 1');
    assert.equal(await evaluate('document.querySelector("#tableFixture input").value'), 'Beta');
    await evaluate('document.getElementById("tableFixture").remove()');

    // Editor tool navigation, focus and user queries survive a data refresh.
    await navigate('/pages/save-editor/index.html');
    await waitFor('typeof window.refreshSavegame === "function"');
    await evaluate(`(async () => {
      document.querySelector('[data-tab=profile]').click();
      await new Promise(resolve => setTimeout(resolve,80));
      const search = document.querySelector('.tool-search'); search.value = 'unlikely-fixture'; search.dispatchEvent(new Event('input',{bubbles:true}));
      document.querySelector('[data-tab=truck]').click();
      await new Promise(resolve => setTimeout(resolve,80));
      document.querySelector('[data-tab=profile]').click();
    })()`);
    await waitFor('document.querySelector(".tool-search").value === "unlikely-fixture"');
    assert.equal(await evaluate('document.querySelector(".tool-search-empty").hidden'), false);
    const selectedTab = await evaluate('document.querySelector(".editor-tabs .nav-btn.active").dataset.tab');
    await evaluate(`window.__fixtureResponses = {refresh_savegame:{data:null,status:'waiting'}}; window.__fixtureCalls=[]; document.dispatchEvent(new KeyboardEvent('keydown',{key:'F5',bubbles:true,cancelable:true}))`);
    await waitFor('!window.saveSync.running && window.__fixtureCalls.includes("refresh_savegame")');
    assert.equal(await evaluate('document.querySelector(".editor-tabs .nav-btn.active").dataset.tab'), selectedTab, 'F5 must refresh instead of navigating');
    await evaluate(`window.__fixtureCalls=[]; window.dispatchEvent(new Event('focus')); document.dispatchEvent(new Event('visibilitychange'))`);
    await waitFor('window.__fixtureCalls.includes("refresh_savegame") && !window.saveSync.running');
    assert.equal(await evaluate('window.__fixtureCalls.filter(command => command === "refresh_savegame").length'), 1);
    assert.equal(await evaluate('document.querySelector(".tool-search").value'), 'unlikely-fixture');
    await evaluate('window.__fixtureResponses = {}; window.notificationManager.clear()');
    const desktopShot = await call("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(path.join(artifacts, `${language}-desktop-regressions.png`), Buffer.from(desktopShot.data, "base64"));

    // Legacy theme migration and persistence across full module initialization.
    for (const [legacy, theme, accent] of [["neon", "dark", "green"], ["neon-red", "dark", "default"], ["unsupported", "dark", "default"], ["light", "light", "default"]]) {
      await evaluate(`localStorage.setItem('theme', ${JSON.stringify(legacy)}); localStorage.removeItem('accent')`);
      await navigate("/pages/save-editor/index.html");
      assert.deepEqual(await evaluate('window.themePreferences.get()'), { theme, accent });
      assert.deepEqual(await evaluate('({ theme: localStorage.getItem("theme"), accent: localStorage.getItem("accent") })'), { theme, accent });
    }
    await evaluate('window.themePreferences.apply({ theme: "dark", accent: "default" })');

    // One workspace choice for a new selection; a valid selection resumes directly.
    await evaluate('localStorage.removeItem("ets2_workspace"); localStorage.setItem("__fixtureBackendMode", "editor")');
    await navigate("/index.html");
    assert.equal(await evaluate('document.querySelector(".launcher-shell").hidden'), false);
    await click("#openSaveEditorBtn");
    await waitFor('location.pathname === "/pages/save-editor/index.html" && document.readyState === "complete"');
    assert.equal(await evaluate('document.getElementById("hubScreen").hidden'), true);
    assert.equal(await evaluate('localStorage.getItem("ets2_workspace")'), "editor");
    await call("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/index.html` });
    await waitFor('location.pathname === "/pages/save-editor/index.html" && document.readyState === "complete"');
    await delay(150);
    await click("#hubHomeBtn");
    await waitFor('location.pathname === "/index.html" && new URL(location.href).searchParams.has("choose") && document.readyState === "complete"');
    assert.equal(await evaluate('document.querySelector(".launcher-shell").hidden'), false);
    await evaluate('localStorage.setItem("ets2_workspace", "invalid")');
    await navigate("/index.html");
    assert.equal(await evaluate('document.querySelector(".launcher-shell").hidden'), false);
    await evaluate('window.__fixtureFailCommand = "hub_set_mode"');
    await click("#openSaveEditorBtn");
    await waitFor('!document.getElementById("openSaveEditorBtn").disabled');
    assert.equal(await evaluate('location.pathname'), "/index.html");
    assert.equal(await evaluate('document.getElementById("launcherWorkspaceError").hidden'), false);
    await evaluate('window.__fixtureFailCommand = null; localStorage.removeItem("ets2_workspace")');
    assert.deepEqual(exceptions, [], "refinement workflows: uncaught errors");
    await call("Page.removeScriptToEvaluateOnNewDocument", { identifier });
  }
  if (checks) console.log(`${checks} page/language/window fixture checks passed; editor tabs checked in both languages.`);
  else console.log("Baseline page checks skipped (--refinement-only).");
  console.log(`${appearanceChecks} theme/accent checks and ${garageChecks} garage volume/window checks passed.`);
  console.log("Switch/checkbox mouse and keyboard states, save failure rollback, theme migration and workspace resume/switch checks passed in both languages.");
  console.log("Desktop regressions passed in both languages: notification layout/queue/dedup/news/duration at four scales, shared refresh/draft preservation, table search/sort, tool navigation and F5/focus refresh.");
  console.log(`Screenshots: ${artifacts}`);
  console.log("Native file dialogs, profile scanning, save writes and real game data are not covered by this fixture.");
} finally {
  socket?.close();
  child.kill();
  server.close();
}
