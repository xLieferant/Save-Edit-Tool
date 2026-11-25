// Real browser UI with synthetic command responses; never opens a game save.
import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../../", import.meta.url));
const sourceRoot = path.join(root, "src");
const artifacts = fs.mkdtempSync(path.join(os.tmpdir(), "ets2-coaches-ui-"));
const chrome = process.env.CHROME_PATH || "C:/Program Files/Google/Chrome/Application/chrome.exe";
const locales = Object.fromEntries(["en", "de"].map((language) => [language,
  JSON.parse(fs.readFileSync(path.join(root, `src-tauri/locales/${language}.json`), "utf8")),
]));
const server = http.createServer((request, response) => {
  const pathname = new URL(request.url, "http://localhost").pathname;
  const file = path.resolve(sourceRoot, `.${decodeURIComponent(pathname)}`);
  if (!file.startsWith(sourceRoot + path.sep) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
    response.writeHead(404).end();
    return;
  }
  const types = { ".js": "text/javascript", ".json": "application/json", ".css": "text/css", ".html": "text/html" };
  response.setHeader("Content-Type", types[path.extname(file)] || "application/octet-stream");
  response.end(fs.readFileSync(file));
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const browser = spawn(chrome, ["--headless", "--disable-gpu", "--no-first-run", "--disable-extensions",
  "--disable-background-networking", "--remote-debugging-port=0", `--user-data-dir=${path.join(artifacts, "profile")}`, "about:blank"],
{ windowsHide: true, stdio: ["ignore", "ignore", "pipe"] });
let socket;
try {
  const endpoint = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("Chrome startup timed out")), 20000);
    let output = "";
    browser.on("error", reject);
    browser.stderr.on("data", (chunk) => {
      output += chunk;
      const match = output.match(/DevTools listening on (ws:\/\/\S+)/);
      if (match) { clearTimeout(timer); resolve(match[1]); }
    });
  });
  const targets = await (await fetch(`http://${new URL(endpoint).host}/json/list`)).json();
  socket = new WebSocket(targets.find((target) => target.type === "page").webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let sequence = 0;
  const pending = new Map();
  const exceptions = [];
  socket.onmessage = ({ data }) => {
    const message = JSON.parse(data);
    if (message.method === "Runtime.exceptionThrown") exceptions.push(message.params.exceptionDetails);
    const request = pending.get(message.id);
    if (!request) return;
    clearTimeout(request.timer);
    pending.delete(message.id);
    if (message.error) request.reject(new Error(JSON.stringify(message.error)));
    else request.resolve(message.result);
  };
  const call = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => reject(new Error(`CDP timeout: ${method}`)), 15000);
    pending.set(id, { resolve, reject, timer });
    socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async (expression) => {
    const result = await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  const waitFor = async (expression) => {
    for (let attempt = 0; attempt < 150; attempt++) {
      try { if (await evaluate(expression)) return; } catch (error) {
        if (!/context|navigat/i.test(error.message)) throw error;
      }
      await new Promise((resolve) => setTimeout(resolve, 40));
    }
    throw new Error(`UI timeout: ${expression}`);
  };
  await call("Page.enable");
  await call("Runtime.enable");
  for (const language of ["de", "en"]) {
    const { identifier } = await call("Page.addScriptToEvaluateOnNewDocument", { source: `
      window.__coachesLocales = ${JSON.stringify(locales)};
      window.__coachesLanguage = ${JSON.stringify(language)};
      window.__coachesReady = false;
      window.addEventListener('translations-ready', () => { window.__coachesReady = true; });
      window.__coachesTranslate = async (key, values = {}) => {
        let text = key.split('.').reduce((value, part) => value?.[part], window.__coachesLocales[window.__coachesLanguage]) ?? key;
        for (const [name, value] of Object.entries(values)) text = text.replaceAll('{' + name + '}', value);
        return text;
      };
      window.__TAURI__ = {
        core: { convertFileSrc: path => path, invoke: async (command, args = {}) => {
          if (window.__syncInvoke && ['refresh_savegame','accept_savegame_refresh','get_selected_game','find_ets2_profiles','find_profile_saves','save_profiles_cache','quicksave_game_info','get_all_trucks','get_player_truck','get_player_trailer'].includes(command)) return window.__syncInvoke(command,args);
          if (window.__saveSelectionStub && ['refresh_savegame','get_selected_game','find_ets2_profiles','find_profile_saves','read_last_profile','save_profiles_cache','set_active_profile','load_profile'].includes(command)) return window.__saveSelectionStub(command,args);
          if (window.__coachesInvoke && ['read_all_save_data', 'read_project_coaches', 'edit_career_xp'].includes(command)) return window.__coachesInvoke(command, args);
          if (command === 'translate_command') return window.__coachesTranslate(args.key, args.args);
          if (command === 'get_current_language_command') return window.__coachesLanguage;
          if (command === 'get_available_languages_command') return [{code:'de',name:'Deutsch'},{code:'en',name:'English'}];
          if (command === 'get_selected_game') return 'ets2';
          if (command === 'hub_get_mode') return 'editor';
          if (/^(find_|list_|scan_|get_available_|get_user_logs|get_user_sessions)/.test(command)) return [];
          return null;
        }},
        event: { listen: async (name, callback) => {
          window.__eventCallbacks ||= {};
          window.__eventCallbacks[name] = callback;
          return () => { delete window.__eventCallbacks[name]; };
        } },
        app: { getVersion: async () => '0.9.9' }, opener: { openUrl: async () => {} },
      };
    ` });
    await call("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/pages/save-editor/index.html` });
    await waitFor("document.readyState === 'complete' && window.__coachesReady && typeof window.invoke === 'function' && typeof window.t === 'function'");
    await evaluate(`(async () => {
      window.__coachesApp = await import('/app.js');
      window.selectedProfilePath = '/synthetic/profile';
      window.selectedSavePath = '/synthetic/profile/save';
      window.currentProfileData = { xp: 999 };
      window.refreshOperationalOverview = async () => {};
      window.t = window.__coachesTranslate;
      window.__coachesWrites = [];
      window.__coachesToasts = [];
      window.showToast = (...args) => window.__coachesToasts.push(args);
      window.__coachesData = {
        saveHash: 'fixture-v1', truckXp: 43863, busXp: 0,
        statistics: Object.fromEntries(['bus_experience_points','bus_total_distance','bus_finished_job_count','bus_cancelled_job_count',
          'bus_total_passengers','bus_total_stops','bus_game_time','bus_playing_time'].map(key => [key, '0'])),
        stops: Array.from({length:131}, (_,i) => ({id:'bus_stop.fixture_' + i, resolved:true, discovered:i<22})),
        discoveredStops:22, declaredStopCount:'131', buses:'0', busReferences:[], currentBusJob:'null', currentBusJobResolved:false,
        myVehiclesMode:'truck', assignedVehiclesMode:'truck', jobLogs:[{id:'log.fixture',version:'0',entries:'0'}], warnings:[],
      };
      window.invoke = async (command, args = {}) => {
        if (command === 'read_all_save_data') return {xp:999,money:1000};
        if (command === 'read_project_coaches') return structuredClone(window.__coachesData);
        if (command === 'edit_career_xp') {
          if (window.__coachesFail) throw new Error(window.__coachesFail);
          if (args.expectedSaveHash !== window.__coachesData.saveHash) throw new Error('coaches.save_changed');
          window.__coachesWrites.push(structuredClone(args));
          if (!window.__coachesBadReadback) window.__coachesData[args.career === 'bus' ? 'busXp' : 'truckXp'] = args.xp;
          window.__coachesData.saveHash += '-next';
          return;
        }
        return window.__TAURI__.core.invoke(command, args);
      };
      window.__coachesInvoke = window.invoke;
      window.__coachesInput = value => { const input = document.getElementById('levelSystemXpInput'); input.value = value; input.dispatchEvent(new Event('input')); };
      await window.__coachesApp.openLevelSystemModal();
    })()`);
    assert.equal(await evaluate("document.getElementById('levelSystemXpInput').value"), "43863", "XP must come from game.sii command");
    await evaluate("window.__coachesInput('50000'); document.getElementById('levelSystemCareerBus').click()");
    assert.equal(await evaluate("document.getElementById('levelSystemXpInput').value"), "0", "unsaved truck input must not reach bus career");
    assert.equal(await evaluate("getComputedStyle(document.querySelector('.level-system-mode-grid')).display"), "none", "bus career must not offer truck levels");
    await evaluate("window.__coachesInput('1234'); document.getElementById('levelSystemCareerTruck').click()");
    assert.equal(await evaluate("document.getElementById('levelSystemXpInput').value"), "50000", "truck draft must survive switching careers");
    assert.equal(await evaluate("document.getElementById('levelSystemCareerTruck').dataset.dirty"), "true");
    await evaluate("document.getElementById('levelSystemCareerBus').click()");
    assert.equal(await evaluate("document.getElementById('levelSystemXpInput').value"), "1234", "bus draft must survive switching careers");
    assert.equal(await evaluate("document.getElementById('levelSystemCurrentXpLabel').textContent"), locales[language].coaches.editor.current_bus_xp);
    assert.equal(await evaluate("document.getElementById('modalLevelSystemApply').textContent"), locales[language].coaches.editor.save_bus);
    assert.equal(await evaluate("document.getElementById('levelSystemCurrentBadge').textContent"), "-", "bus career has no inferred truck level");
    assert.equal(await evaluate("document.getElementById('levelSystemBusStatsGrid').childElementCount"), 7);
    await evaluate("document.getElementById('levelSystemCareerBus').dispatchEvent(new KeyboardEvent('keydown', {key:'ArrowLeft',bubbles:true}))");
    assert.equal(await evaluate("document.getElementById('levelSystemCareerTruck').getAttribute('aria-selected')"), "true");
    assert.equal(await evaluate("document.getElementById('modalLevelSystemApply').textContent"), locales[language].coaches.editor.save_truck);
    assert.equal(await evaluate("document.getElementById('levelSystemTruckProgress').hidden"), false);
    assert.equal(await evaluate("document.getElementById('levelSystemProgressBar').value > 0"), true);
    await evaluate("document.getElementById('levelSystemCareerBus').click(); window.__coachesInput('')");
    assert.equal(await evaluate("document.getElementById('modalLevelSystemApply').disabled"), true, "blank XP must be rejected");
    await evaluate("document.getElementById('levelSystemCareerTruck').click()");
    assert.equal(await evaluate("document.getElementById('levelSystemXpInput').value"), "50000", "invalid bus input must not affect truck draft");
    await evaluate("document.getElementById('levelSystemCareerBus').click()");
    assert.equal(await evaluate("document.getElementById('levelSystemXpInput').value"), "", "invalid input is retained in its own career");
    assert.equal(await evaluate("document.getElementById('modalLevelSystemApply').disabled"), true);
    for (const value of ["-1", "1.5", "9007199254740992"]) {
      await evaluate(`window.__coachesInput(${JSON.stringify(value)})`);
      assert.equal(await evaluate("document.getElementById('modalLevelSystemApply').disabled"), true, `invalid bus XP: ${value}`);
    }
    await evaluate("window.__coachesInput('10000000000')");
    assert.equal(await evaluate("document.getElementById('modalLevelSystemApply').disabled"), false, "bus XP must not use truck level cap");
    await evaluate("window.__coachesInput('1234')");
    // Exercise the real modal in both themes, at the app minimum and a large window.
    for (const [width, height] of [[900, 640], [1600, 1000]]) {
      await call("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
      for (const theme of ["dark", "light"]) {
        await evaluate(`window.themePreferences.apply({theme:${JSON.stringify(theme)},accent:'green'})`);
        for (const career of ["truck", "bus"]) {
          await evaluate(`document.getElementById(${JSON.stringify(career === "truck" ? "levelSystemCareerTruck" : "levelSystemCareerBus")}).click()`);
          const expectedBackground = career === "truck" ? "rgb(124, 159, 195)" : "rgb(143, 175, 159)";
          await waitFor(`getComputedStyle(document.getElementById('modalLevelSystemApply')).backgroundColor === ${JSON.stringify(expectedBackground)}`);
          // The apply button settles before the tab/card color transitions.
          // Measure their final contrast after those finite transitions finish.
          await evaluate(`(async () => {
            await new Promise(requestAnimationFrame);
            await Promise.all(document.getElementById('modalLevelSystem').getAnimations({subtree:true})
              .filter(animation => animation.effect?.getTiming().iterations !== Infinity)
              .map(animation => animation.finished.catch(() => {})));
          })()`);
          assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth + 1"), false);
          assert.equal(await evaluate("document.querySelector('#modalLevelSystem .level-system-scroll').scrollWidth > document.querySelector('#modalLevelSystem .level-system-scroll').clientWidth + 1"), false, "modal must not overflow horizontally");
          assert.equal(await evaluate("document.querySelector('#modalLevelSystem .modal-box').getBoundingClientRect().bottom <= innerHeight + 1"), true, "footer must stay inside the window");
          assert.equal(await evaluate("getComputedStyle(document.getElementById('modalLevelSystem')).getPropertyValue('--accent').trim()"), career === "truck" ? "#7c9fc3" : "#8faf9f", "career accents must override the global green preference locally");
          const buttonColors = await evaluate("(() => {const n=document.getElementById('modalLevelSystemApply'),s=getComputedStyle(n);return {background:s.backgroundColor,accent:s.getPropertyValue('--accent').trim(),transition:s.transition,inside:!!n.closest('#modalLevelSystem'),attributes:n.getAttribute('style')};})()");
          assert.equal(buttonColors.background, career === "truck" ? "rgb(124, 159, 195)" : "rgb(143, 175, 159)", JSON.stringify(buttonColors));
          const contrast = await evaluate(`(() => {
            const rgb = color => (color.match(/[\\d.]+/g) || []).map(Number);
            const light = color => color.slice(0, 3).map(v => v / 255).map(v => v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4).reduce((sum,v,i) => sum + v * [.2126,.7152,.0722][i],0);
            const background = node => {
              const parents = []; for(let p=node;p;p=p.parentElement) parents.unshift(p);
              return parents.reduce((surface,p) => {const c=rgb(getComputedStyle(p).backgroundColor); const a=c[3]??1; return surface.map((v,i)=>c[i]*a+v*(1-a));},[28,32,37]);
            };
            return [...document.querySelectorAll('#modalLevelSystem button:not(:disabled), #modalLevelSystem .coaches-career-copy small, #modalLevelSystem .coaches-career-dirty, #levelSystemCareerBadge')]
              .filter(n=>n.getClientRects().length && getComputedStyle(n).visibility!=='hidden').map(n=>{const a=light(rgb(getComputedStyle(n).color)),b=light(background(n));return {id:n.id||n.className,ratio:(Math.max(a,b)+.05)/(Math.min(a,b)+.05)};}).filter(n=>n.ratio<4.5);
          })()`);
          assert.deepEqual(contrast, [], `${theme}/${career}: readable text contrast`);
          const screenshot = await call("Page.captureScreenshot", { format: "png" });
          fs.writeFileSync(path.join(artifacts, `modal-${language}-${theme}-${career}-${width}.png`), Buffer.from(screenshot.data, "base64"));
        }
      }
    }
    await evaluate("document.getElementById('modalLevelSystemApply').click()");
    await waitFor("document.getElementById('modalLevelSystemApply').disabled && !document.getElementById('levelSystemCareerBus').disabled");
    assert.equal(await evaluate("document.getElementById('modalLevelSystem').style.display"), "flex", "saving one career must keep the other draft accessible");
    assert.deepEqual(await evaluate("window.__coachesWrites.map(({career,xp})=>({career,xp}))"), [{ career: "bus", xp: 1234 }]);
    assert.equal(await evaluate("window.__coachesData.truckXp"), 43863);
    assert.equal(await evaluate("window.currentProfileData.xp"), 43863, "bus edits must preserve the displayed truck XP");
    assert.equal(await evaluate("window.__coachesToasts.at(-1)[0]"), "coaches.editor.saved_bus");
    assert.equal(await evaluate("document.getElementById('levelSystemCareerBus').dataset.dirty"), "false");

    await evaluate("document.getElementById('levelSystemCareerTruck').click()");
    assert.equal(await evaluate("document.getElementById('levelSystemXpInput').value"), "50000", "saving bus XP must not discard truck draft");
    await evaluate("document.getElementById('modalLevelSystemApply').click()");
    await waitFor("document.getElementById('modalLevelSystemApply').disabled && !document.getElementById('levelSystemCareerTruck').disabled");
    assert.equal(await evaluate("window.__coachesData.busXp"), 1234);
    assert.equal(await evaluate("window.__coachesData.truckXp"), 50000);
    assert.equal(await evaluate("window.__coachesToasts.at(-1)[0]"), "coaches.editor.saved_truck");
    await evaluate("window.loadProfileData()");
    assert.equal(await evaluate("window.currentProfileData.xp"), 50000, "reload must use economy XP rather than stale info.sii metadata");
    // The existing skill modal remains available without discarding XP drafts.
    await evaluate("window.__coachesInput('51000'); document.getElementById('levelSystemSkills').click()");
    await waitFor("document.getElementById('modalSkilltree').style.display === 'flex' && document.querySelector('#skilltreeModalRoot .skilltree-surface')");
    await evaluate("window.__coachesData.saveHash += '-skills'; document.getElementById('modalSkilltreeClose').click()");
    await waitFor("!document.getElementById('levelSystemCareerTruck').disabled");
    assert.equal(await evaluate("document.getElementById('levelSystemXpInput').value"), "51000", "skills must not discard XP drafts");

    await evaluate("document.getElementById('modalLevelSystemClose').click(); window.__coachesData.busXp = null; window.__coachesApp.openLevelSystemModal('bus')");
    assert.equal(await evaluate("document.getElementById('levelSystemXpInput').disabled && document.getElementById('modalLevelSystemApply').disabled"), true);
    await evaluate("document.getElementById('levelSystemCareerTruck').click()");
    assert.equal(await evaluate("document.getElementById('levelSystemXpInput').disabled"), false, "old saves must retain truck editing");
    await evaluate("document.getElementById('modalLevelSystemClose').click(); window.__coachesData.busXp = 1234; window.__coachesApp.openLevelSystemModal('bus')");
    await evaluate("window.__coachesFail = 'coaches.save_changed'; window.__coachesInput('2222'); document.getElementById('modalLevelSystemApply').click()");
    await waitFor("!document.getElementById('levelSystemCareerBus').disabled");
    assert.equal(await evaluate("document.getElementById('levelSystemHint').textContent"), locales[language].coaches.save_changed);
    assert.equal(await evaluate("window.__coachesWrites.length"), 2, "rejected writes must not mutate XP");
    await evaluate("document.getElementById('modalLevelSystemClose').click(); window.__coachesFail = null; window.__coachesApp.openLevelSystemModal('bus')");
    await evaluate("window.__coachesBadReadback = true; window.__coachesInput('3333'); document.getElementById('modalLevelSystemApply').click()");
    await waitFor("!document.getElementById('levelSystemCareerBus').disabled");
    assert.equal(await evaluate("document.getElementById('levelSystemHint').textContent"), locales[language].coaches.verify_failed);
    await evaluate("document.getElementById('modalLevelSystemClose').click(); window.__coachesBadReadback = false; document.querySelector('.editor-tabs [data-tab=coaches]').click()");
    await waitFor("document.querySelectorAll('[data-coaches-stops] li').length === 131");
    assert.equal(await evaluate("document.querySelector('.editor-tabs [data-tab=coaches]').classList.contains('active')"), true);
    await waitFor(`document.getElementById('editorStageTitle').textContent === ${JSON.stringify(locales[language].coaches.title)}`);
    assert.equal(await evaluate("document.querySelectorAll('[data-coaches-stops] li').length"), 131);
    assert.match(await evaluate("document.querySelector('[data-coaches-stop-counts]').textContent"), /22 \/ 131/);
    for (const [width, height] of [[900, 640], [1600, 1000]]) {
      await call("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
      for (const theme of ["dark", "light"]) {
        await evaluate(`window.themePreferences.apply({theme:${JSON.stringify(theme)},accent:'default'})`);
        assert.equal(await evaluate("document.documentElement.scrollWidth > innerWidth + 1"), false, "Coaches page must not overflow");
        const screenshot = await call("Page.captureScreenshot", { format: "png" });
        fs.writeFileSync(path.join(artifacts, `${language}-${theme}-${width}.png`), Buffer.from(screenshot.data, "base64"));
      }
    }
    await evaluate("window.__coachesData.stops[0].id = '<img src=x onerror=alert(1)>'; window.__coachesApp.loadTools('coaches')");
    assert.equal(await evaluate("document.querySelector('[data-coaches-stops] img') === null"), true, "save IDs must be rendered as text");
    await evaluate(`(async () => {
      window.__moneyTools = await import('/tools.js');
      window.__moneyTool = window.__moneyTools.tools.profile.find(tool => tool.title === 'tools.profile.change_money.title');
      window.__moneyOriginalInvoke = window.invoke;
      window.__moneyOriginalBridge = window.__coachesInvoke;
      window.__moneyPersisted = 1000;
      window.__moneyMode = 'persist';
      window.__moneyCalls = [];
      window.invoke = async (command, args = {}) => {
        if (command === 'edit_player_money') {
          window.__moneyCalls.push('write');
          if (window.__moneyMode === 'reject') throw new Error('save_path_invalid');
          if (window.__moneyMode === 'persist') window.__moneyPersisted = args.value;
          return;
        }
        if (command === 'read_all_save_data') {
          window.__moneyCalls.push('read');
          return {money:window.__moneyPersisted,xp:window.__coachesData.truckXp};
        }
        return window.__moneyOriginalInvoke(command,args);
      };
      window.__coachesInvoke = window.invoke;
      window.currentProfileData = {money:1000,xp:window.__coachesData.truckXp};
      void window.__moneyTool.action();
    })()`);
    await waitFor("document.getElementById('modalNumber').style.display === 'flex'");
    await evaluate("document.getElementById('modalNumberInput').value = '1234567'; document.getElementById('modalNumberApply').click()");
    await waitFor("window.__coachesToasts.at(-1)?.[0] === 'game_units.money_saved'");
    assert.equal(await evaluate("window.currentProfileData.money"),1234567);
    assert.deepEqual(await evaluate("window.__moneyCalls"),['write','read']);
    for (const mode of ['mismatch','reject']) {
      await evaluate(`window.__moneyMode = ${JSON.stringify(mode)}; window.__moneyCalls = []; void window.__moneyTool.action()`);
      await waitFor("document.getElementById('modalNumber').style.display === 'flex'");
      await evaluate("document.getElementById('modalNumberInput').value = '7654321'; document.getElementById('modalNumberApply').click()");
      await waitFor("window.__coachesToasts.at(-1)?.[0] === 'toasts.change_money_error'");
      assert.equal(await evaluate("window.currentProfileData.money"),1234567, 'failed persistence must never assign the requested value');
      assert.deepEqual(await evaluate("window.__moneyCalls"),mode === 'reject' ? ['write'] : ['write','read']);
    }
    await evaluate("window.invoke = window.__moneyOriginalInvoke; window.__coachesInvoke = window.__moneyOriginalBridge");
    await evaluate(`(async () => {
      window.__atsUnits = await import('/modules/shared/game_units.js');
      window.selectedGame = 'ats';
      window.readSaveGameConfig = {use_miles:true};
    })()`);
    assert.equal(await evaluate("window.__atsUnits.currencyCode()"), "USD");
    assert.equal(await evaluate("window.__atsUnits.distanceUnit()"), "mi");
    assert.equal(await evaluate("window.__atsUnits.displayDistance(1.609344)"), 1);
    assert.equal(await evaluate("window.__atsUnits.storedDistance(1)"), 1.609344);
    await evaluate("window.readSaveGameConfig.use_miles = false");
    assert.equal(await evaluate("window.__atsUnits.displayDistance(6500)"), 6500, "ATS metric setting must retain stored kilometers");
    await evaluate(`(async () => {
      window.__atsOriginalInvoke = window.invoke;
      window.__atsWritable = true;
      window.__atsGarageCalls = [];
      window.invoke = async (command, args = {}) => {
        window.__atsGarageCalls.push(command);
        if (command === 'get_all_garages') return {game:'ats', writeSupported:window.__atsWritable, aiDriversSupported:true, saveHash:'ats-fixture', diagnostics:{warnings:[]}, garages:[{
          garageId:'garage.los_angeles', cityToken:'los_angeles', cityName:'Los Angeles', ownership:'not_owned', size:'unowned', status:0,
          vehicleSlotCount:0,driverSlotCount:0,trailerSlotCount:0,maximumSlotCount:0,occupiedSlots:0,availableSlots:0,
          assignedDriverCount:0,assignedTruckCount:0,assignedTrailerCount:0,slots:[],trailerIds:[],isHeadquarters:false,capacityConsistent:true,warnings:[],
        }]};
        if (command === 'get_ai_driver_management') return {game:'ats',saveHash:'ats-fixture',drivers:[],availableTrucks:[],availableDriverCount:0,warnings:[]};
        return window.__atsOriginalInvoke(command,args);
      };
      await window.__coachesApp.loadTools('garages');
    })()`);
    await waitFor("document.querySelector('[data-garage-action=purchase]') && !document.querySelector('[data-garage-action=purchase]').disabled");
    assert.equal(await evaluate("window.__atsGarageCalls.includes('get_ai_driver_management')"), true, "ATS AI inventory must load");
    await evaluate("window.__atsWritable = false; window.__coachesApp.loadTools('garages')");
    await waitFor("document.querySelector('[data-garage-action=purchase]')?.disabled");
    await evaluate("window.invoke = window.__atsOriginalInvoke; window.selectedGame = 'ets2'; window.readSaveGameConfig = {use_miles:false}");
    assert.equal(await evaluate("window.__atsUnits.currencyCode()"), "EUR");
    assert.equal(await evaluate("window.__atsUnits.displayDistance(6500)"), 6500);
    await evaluate(`(() => {
      window.__selectionBackendSave = '/synthetic/profile/save/old';
      window.__saveSelectionStub = async (command,args) => {
        if (command === 'refresh_savegame') return {...args,status:'waiting',data:null};
        if (command === 'get_selected_game') return 'ets2';
        if (command === 'find_ets2_profiles') return [{path:'/synthetic/profile',name:'Synthetic',success:true}];
        if (command === 'read_last_profile') return '/synthetic/profile';
        if (command === 'find_profile_saves') return [{path:'/synthetic/profile/save/missing',folder:'7',name:'Missing slot',success:true,kind:'Manual'}];
        if (command === 'set_active_profile') { window.__selectionBackendSave = null; return; }
        if (command === 'load_profile') throw new Error('game_sii_not_found');
        return;
      };
      document.getElementById('refreshBtn').click();
    })()`);
    await waitFor("document.querySelector('[data-dropdown-value=\"/synthetic/profile/save/missing\"]')");
    await evaluate(`window.selectedSavePath = '/synthetic/profile/save/old'; window.currentSavePath = window.selectedSavePath; window.__selectionBackendSave = window.selectedSavePath;
      document.querySelector('[data-dropdown-value="/synthetic/profile/save/missing"]').click()`);
    await waitFor("window.__coachesToasts.at(-1)?.[0] === 'toasts.save_load_error'");
    assert.equal(await evaluate("window.selectedSavePath"),null, 'failed selection must not become active in the UI');
    assert.equal(await evaluate("window.currentSavePath"),null);
    assert.equal(await evaluate("window.__selectionBackendSave"),null, 'failed selection must not leave the previous save writable');
    await evaluate("window.__moneyTool.action()");
    assert.equal(await evaluate("window.__coachesToasts.at(-1)?.[0]"),'toasts.change_money_error', 'a cleared save selection must never allow another write');
    assert.equal(await evaluate("document.getElementById('modalNumber').style.display"),'none');
    await evaluate("delete window.__saveSelectionStub");
    await evaluate(`(async () => {
      window.selectedProfilePath = '/synthetic/live';
      window.selectedSavePath = '/synthetic/live/save/7';
      window.currentSavePath = window.selectedSavePath;
      window.currentProfileData = {money:6500,xp:100};
      window.__eventCallbacks['career://game_running']({payload:true});
      window.__eventCallbacks['career://bridge_connected']({payload:false});
      window.__syncCalls = [];
      window.__syncMoney = 12000;
      window.__syncFail = false;
      window.__syncDelay = 0;
      window.__syncInvoke = async (command,args) => {
        window.__syncCalls.push(command);
        if (command === 'get_selected_game') return 'ets2';
        if (command === 'find_ets2_profiles') return [{path:'/synthetic/live',name:'Live',success:true}];
        if (command === 'find_profile_saves') return [{path:'/synthetic/live/save/7',folder:'7',name:'Live slot',success:true,kind:'Manual'},
          {path:'/synthetic/live/save/autosave',folder:'autosave',name:'New autosave',success:true,kind:'Autosave'}];
        if (command === 'refresh_savegame') {
          if (window.__syncDelay) await new Promise(resolve => setTimeout(resolve,window.__syncDelay));
          if (window.__syncFail) throw new Error('save_read_failed: sharing violation');
          return {...args,revision:'live-v' + window.__syncMoney,data:{money:window.__syncMoney,xp:200},status:'synchronized'};
        }
        if (command === 'quicksave_game_info') return {adr:1,long_dist:2};
        if (command === 'get_all_trucks') return [{truck_id:'live-truck'}];
        if (command === 'get_player_truck') return {truck_id:'live-truck'};
        if (command === 'get_player_trailer') return {has_trailer:true,trailer:{trailer_id:'live-trailer'},has_active_job:true};
        return null;
      };
      await window.__coachesApp.loadTools('profile');
      await window.refreshSavegame({manual:true});
    })()`);
    assert.equal(await evaluate("window.currentProfileData.money"),12000);
    assert.equal(await evaluate("document.getElementById('statusGameRunning').classList.contains('is-active')"),true,'game status stays active while refresh succeeds');
    assert.equal(await evaluate("document.getElementById('statusSdkConnected').classList.contains('is-active')"),false,'refresh does not require an SDK connection');
    assert.equal(await evaluate("window.currentQuicksaveData.long_dist"),2);
    assert.equal(await evaluate("window.playerTruck.truck_id"),'live-truck');
    assert.equal(await evaluate("window.playerTrailer.trailer_id"),'live-trailer');
    assert.equal(await evaluate("window.playerTrailerHasActiveJob"),true);
    assert.equal(await evaluate("window.selectedSavePath"),'/synthetic/live/save/7');
    assert.equal(await evaluate("document.getElementById('saveSyncStatus').dataset.state"),'synchronized');
    assert.equal(await evaluate("window.__coachesApp.activeTab"),'profile');
    assert.equal(await evaluate("!!document.querySelector('[data-dropdown-value=\"/synthetic/live/save/autosave\"]')"),true);
    // An external change while a modal contains a draft must not read or accept it.
    await evaluate(`(() => {
      window.__syncMoney = 18000;
      const modal = document.getElementById('modalNumber');
      modal.style.display = 'flex';
      const input = modal.querySelector('input');
      input.value = '98765'; input.dispatchEvent(new Event('input',{bubbles:true}));
      window.__syncBefore = window.__syncCalls.filter(command => command === 'refresh_savegame').length;
      window.__eventCallbacks['savegame://changed']({payload:{game:'ets2',profilePath:window.selectedProfilePath,savePath:window.selectedSavePath}});
    })()`);
    await waitFor("!window.saveSync.running && document.getElementById('saveSyncStatus').dataset.state === 'changed'");
    assert.equal(await evaluate("window.currentProfileData.money"),12000);
    assert.equal(await evaluate("document.getElementById('modalNumber').querySelector('input').value"),'98765');
    assert.equal(await evaluate("window.__syncCalls.filter(command => command === 'refresh_savegame').length === window.__syncBefore"),true);
    await evaluate("document.getElementById('modalNumber').style.display = 'none'");
    await waitFor("window.currentProfileData.money === 18000 && !window.saveSync.running");
    // Bursts share a single in-flight service call and preserve the selected slot.
    await evaluate("window.__syncMoney = 20000; window.__syncDelay = 100; window.__syncPromises = [window.refreshSavegame(),window.refreshSavegame(),window.refreshSavegame()]");
    assert.equal(await evaluate("window.__syncPromises.every(promise => promise === window.__syncPromises[0])"),true);
    await waitFor("window.currentProfileData.money === 20000 && !window.saveSync.running");
    await evaluate("window.__syncDelay = 0; window.__syncFail = true; window.refreshSavegame({manual:true})");
    await waitFor("!window.saveSync.running && document.getElementById('saveSyncStatus').dataset.state === 'unavailable'");
    assert.equal(await evaluate("window.currentProfileData.money"),20000,'temporary errors retain last good data');
    await waitFor("window.saveSync.retries === 3 && !window.saveSync.running");
    assert.equal(await evaluate("window.currentProfileData.money"),20000,'bounded retries must retain the displayed snapshot');
    await evaluate("window.__syncFail = false; window.__syncMoney = 23000; window.refreshSavegame({manual:true})");
    await waitFor("window.currentProfileData.money === 23000 && !window.saveSync.running");
    // A result for a different profile must never replace the newly selected UI.
    await evaluate("window.__syncDelay = 300; window.__syncMoney = 99000; void window.refreshSavegame();");
    await waitFor("window.saveSync.running && window.__syncCalls.at(-1) === 'refresh_savegame'");
    await evaluate("window.selectedProfilePath = '/synthetic/other-profile'");
    await waitFor("!window.saveSync.running");
    assert.equal(await evaluate("window.currentProfileData.money"),23000);
    await evaluate("window.selectedProfilePath = '/synthetic/live'; window.__syncDelay = 0; window.__syncMoney = 24000; window.refreshSavegame({manual:true})");
    await waitFor("window.currentProfileData.money === 24000 && !window.saveSync.running");
    // Search inputs and selected garage filters survive an active view refresh.
    await evaluate(`(async () => {
      window.__atsWritable = true;
      window.__atsOriginalInvoke = window.invoke;
      window.invoke = async (command,args) => {
        if (command === 'get_all_garages') return {game:'ets2',writeSupported:true,aiDriversSupported:true,saveHash:'live',diagnostics:{warnings:[]},garages:[]};
        if (command === 'get_ai_driver_management') return {game:'ets2',saveHash:'live',drivers:[],availableTrucks:[],availableDriverCount:0,warnings:[]};
        return window.__atsOriginalInvoke(command,args);
      };
      await window.__coachesApp.loadTools('garages');
      const search = document.querySelector('[data-garage-search=citySearch]');
      search.value = 'Berlin'; search.dispatchEvent(new Event('input',{bubbles:true}));
      const filter = document.querySelector('[data-garage-filter=ownership]');
      filter.value = 'owned'; filter.dispatchEvent(new Event('change',{bubbles:true}));
      await window.refreshSavegame({manual:true});
    })()`);
    assert.equal(await evaluate("window.__coachesApp.activeTab"),'garages');
    assert.equal(await evaluate("document.querySelector('[data-garage-search=citySearch]').value"),'Berlin');
    assert.equal(await evaluate("document.querySelector('[data-garage-filter=ownership]').value"),'owned');
    await evaluate("window.invoke = window.__atsOriginalInvoke; delete window.__syncInvoke");
    assert.deepEqual(exceptions, [], "uncaught browser errors");
    await call("Page.removeScriptToEvaluateOnNewDocument", { identifier });
  }
  console.log("Money persistence/readback, rejected writes and mismatched reloads passed in DE/EN.\nATS currency, distance preferences, garage capabilities and AI inventory passed in DE/EN.\nCoaches browser tests passed in DE/EN: independent drafts and validation, keyboard tabs, career labels/accents, save/readback, legacy saves, 131/22 stops and responsive dark/light layouts.");
  console.log(`Screenshots: ${artifacts}`);
  console.log("Live synchronization UI passed in DE/EN: shared manual/event refresh, values/vehicles/skills/job context, new saves, draft conflicts, burst serialization, temporary errors, profile isolation and preserved filters.");
} finally {
  socket?.close();
  browser.kill();
  await new Promise((resolve) => server.close(resolve));
}
