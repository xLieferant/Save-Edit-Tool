// Browser checks for the active user-log dialog. All fixture data is synthetic.
import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../../../", import.meta.url));
const sourceRoot = path.join(root, "src");
const artifacts = fs.mkdtempSync(path.join(os.tmpdir(), "ets2-user-log-ui-"));
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
const native = process.argv.includes("--native");
const browser = native ? null : spawn(chrome, ["--headless", "--disable-gpu", "--no-first-run", "--disable-extensions",
  "--disable-background-networking", "--remote-debugging-port=0", `--user-data-dir=${path.join(artifacts, "profile")}`, "about:blank"],
{ windowsHide: true, stdio: ["ignore", "ignore", "pipe"] });
let socket;
try {
  const endpoint = native ? (await (await fetch(`http://127.0.0.1:${process.env.ETS2_DIAGNOSTIC_CDP_PORT || 19223}/json/version`)).json()).webSocketDebuggerUrl : await new Promise((resolve, reject) => {
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
  if (native) {
    await waitFor("Boolean(window.__TAURI__?.core?.invoke)");
    const result = await evaluate(`(async () => {
      const invoke = (...args) => window.__TAURI__.core.invoke(...args);
      const language = await invoke('get_current_language_command');
      const {openLogFile} = await import('/modules/shared/diagnostics_ui.js');
      try {
        for (const level of ['INFO','WARN','ERROR']) await invoke('diagnostics_record', {event:{level,module:'user_log_validation',action:'native_smoke',message:'User log native validation ' + level,technical:'Permission denied; token=native_validation_secret'}});
        const snapshots = [];
        for (const value of ['de','en']) {
          await invoke('set_language_command', {language:value});
          await Promise.all([openLogFile(), openLogFile()]);
          const dialogs = document.querySelectorAll('dialog.log-file-dialog[open]');
          if(dialogs.length!==1) throw new Error('Expected one dialog');
          const dialog=dialogs[0];
          if(dialog.querySelectorAll('.log-file-meta-card').length!==4) throw new Error('Missing metadata cards');
          dialog.querySelector('[data-log-filter=ERROR]').click();
          const entries=[...dialog.querySelectorAll('.log-file-entry')];
          if(!entries.length || entries.some(n=>!['ERROR','FATAL'].includes(n.dataset.level))) throw new Error('Wrong error filter');
          if(dialog.textContent.includes('native_validation_secret')) throw new Error('Secret in viewer');
          snapshots.push({language:value,title:dialog.querySelector('h2').textContent,filters:dialog.querySelectorAll('[data-log-filter]').length,actions:dialog.querySelectorAll('.log-file-actions button').length});
          dialog.close();
          await new Promise(resolve=>setTimeout(resolve,20));
        }
        return snapshots;
      } finally {document.querySelector('dialog.log-file-dialog[open]')?.close(); await invoke('set_language_command',{language});}
    })()`);
    assert.equal(result.length, 2);
    assert.deepEqual(result.map(row => [row.filters, row.actions]), [[4, 2], [4, 2]]);
    console.log("Native WebView log-viewer checks passed in DE/EN.");
    if (process.argv.includes("--export")) {
      const exported = await evaluate(`(async()=>{
        const {openLogFile}=await import('/modules/shared/diagnostics_ui.js');
        try {
          await openLogFile();
          const dialog=document.querySelector('.log-file-dialog');
          dialog.querySelector('[data-log-filter=ERROR]').click();
          const button=dialog.querySelector('.log-file-export');
          button.click();
          await new Promise((resolve,reject)=>{let attempts=0;const check=()=>{if(!button.disabled)return resolve();if(++attempts>300)return reject(new Error('Native export timed out'));setTimeout(check,30);};check();});
          const feedback=dialog.querySelector('.log-file-feedback');
          if(feedback.dataset.state==='error') throw new Error('Native export failed: '+feedback.textContent);
          const path=feedback.title;
          const version=await window.__TAURI__.app.getVersion();
          return {path,version};
        } finally {document.querySelector('dialog.log-file-dialog[open]')?.close();}
      })()`);
      assert(exported.path, "Native export must return a written file");
      assert.match(path.basename(exported.path), /^SaveEditTool-UserLogs_\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}(?:_\d+)?\.log$/);
      const report=fs.readFileSync(exported.path,"utf8");
      assert(report.includes(`Version: ${exported.version}`));
      for(const level of ["INFO","WARN","ERROR"]) assert(report.includes(`User log native validation ${level}`));
      assert(!report.includes("native_validation_secret"));
      assert(report.includes("Log Entries:") && report.includes("Export Date:") && report.includes("Operating System:"));
      console.log("Native one-click .log export passed: filename, installed version, all levels with active ERROR filter and secret masking.");
    }
  } else {
  for (const language of ["de", "en"]) {
    const {identifier} = await call("Page.addScriptToEvaluateOnNewDocument", {source: `
      window.__logLanguage=${JSON.stringify(language)};
      window.__logLocales=${JSON.stringify(locales)};
      window.__logFixture={revision:'one',readCalls:0,exports:0,failRead:false,failExport:false,cancelExport:false};
      window.__logFixture.entries=Array.from({length:900},(_,i)=>{
        const level=['ERROR','WARN','INFO'][i%3];
        const timestamp='2026-11-14T11:46:32+01:00';
        return {timestamp,level,text:'['+timestamp+'] ['+level+'] [fixture] entry-'+i+(i===0?' '+ 'long-line-'.repeat(1600):'')+'\\n  Reason: preserved context\\n'};
      });
      window.__TAURI__={
        core:{convertFileSrc:p=>p,invoke:async(command,args={})=>{
          const f=window.__logFixture;
          if(command==='translate_command') return args.key.split('.').reduce((v,k)=>v?.[k],window.__logLocales[window.__logLanguage])??args.key;
          if(command==='get_current_language_command') return window.__logLanguage;
          if(command==='get_available_languages_command') return [{code:'de',name:'Deutsch'},{code:'en',name:'English'}];
          if(command==='get_selected_game') return 'ets2';
          if(command==='hub_get_mode') return 'editor';
          if(command==='diagnostics_user_logs') {
            f.readCalls++;
            if(f.failRead) throw new Error('user_log_report.errors.read_failed');
            if(args.knownRevision===f.revision) return null;
            return {revision:f.revision,logFilePath:'C:/Users/Fixture/'+ 'very-long-directory/'.repeat(40)+'application.log.txt',sizeBytes:1600000,lastModified:'2026-11-14T11:46:32+01:00',warningCount:300,errorCount:300,entries:structuredClone(f.entries)};
          }
          if(command==='export_log_file') {
            f.exports++;
            if(f.failExport) throw new Error('user_log_report.errors.write_failed');
            if(f.cancelExport) return null;
            f.exportText=f.entries.map(e=>e.text).join('');
            return 'C:/Users/Fixture/Downloads/SaveEditTool-UserLogs_2026-11-14_11-46-32.log';
          }
          if(/^(find_|list_|scan_|get_available_|get_user_logs|get_user_sessions)/.test(command)) return [];
          return null;
        }},
        event:{listen:async()=>()=>{}},app:{getVersion:async()=>'0.9.9'},opener:{openUrl:async()=>{}},
      };
    `});
    await call("Page.navigate", {url:`http://127.0.0.1:${server.address().port}/pages/save-editor/index.html`});
    await waitFor("document.readyState==='complete' && typeof window.openUserLogsModal==='function'");
    await evaluate("Promise.all([window.openUserLogsModal(),window.openUserLogsModal()])");
    assert.equal(await evaluate("document.querySelectorAll('dialog.log-file-dialog[open]').length"), 1);
    assert.equal(await evaluate("document.querySelector('.log-file-dialog h2').textContent"), locales[language].user_log_report.title);
    assert.equal(await evaluate("document.querySelectorAll('.log-file-meta-card').length"), 4);
    assert.equal(await evaluate("document.querySelector('[data-log-meta=errors]').textContent"), "300");
    assert.equal(await evaluate("document.querySelector('[data-log-meta=warnings]').textContent"), "300");
    assert.equal(await evaluate("document.querySelector('.log-file-path').textContent===document.querySelector('.log-file-path').title"), true);
    assert.equal(await evaluate("document.querySelectorAll('.log-file-entry').length"), 250);
    const reads = await evaluate("window.__logFixture.readCalls");
    for (const level of ["INFO","WARN","ERROR","all"]) {
      await evaluate(`document.querySelector('[data-log-filter=${level}]').click()`);
      assert.equal(await evaluate(`!!document.querySelector('.log-file-entry') && [...document.querySelectorAll('.log-file-entry')].every(n=>${JSON.stringify(level)}==='all'||n.dataset.level===${JSON.stringify(level)})`), true);
    }
    assert.equal(await evaluate("window.__logFixture.readCalls"), reads, "filters must not query the backend");
    await evaluate("document.querySelector('.log-file-preview').scrollTop=document.querySelector('.log-file-preview').scrollHeight; document.querySelector('.log-file-preview').dispatchEvent(new Event('scroll'))");
    assert.ok(await evaluate("document.querySelectorAll('.log-file-entry').length") > 250, "scrolling reveals further cached entries");

    for (const [width,height] of [[900,640],[1210,800],[1600,1000],[480,640]]) {
      await call("Emulation.setDeviceMetricsOverride",{width,height,deviceScaleFactor:1,mobile:false});
      for (const theme of ["dark","light"]) {
        await evaluate(`window.themePreferences.apply({theme:${JSON.stringify(theme)},accent:'default'}); document.querySelector('.log-file-preview').scrollTop=0`);
        const layout=await evaluate(`(()=>{
          const d=document.querySelector('.log-file-dialog'),viewer=d.querySelector('.log-file-preview'),footer=d.querySelector('.log-file-footer'),cards=d.querySelector('.log-file-metadata');
          const r=d.getBoundingClientRect(),v=viewer.getBoundingClientRect(),f=footer.getBoundingClientRect(),c=cards.getBoundingClientRect();
          return {within:r.left>=0&&r.right<=innerWidth+1&&r.top>=0&&r.bottom<=innerHeight+1,noOverflow:d.scrollWidth<=d.clientWidth+1,viewerScroll:viewer.scrollHeight>viewer.clientHeight,ordered:c.bottom<=v.top&&v.bottom<=f.top+1,footerVisible:f.bottom<=r.bottom+1,buttons:[...d.querySelectorAll('.log-file-actions button')].every(n=>n.getBoundingClientRect().bottom<=r.bottom+1)};
        })()`);
        assert.deepEqual(layout,{within:true,noOverflow:true,viewerScroll:true,ordered:true,footerVisible:true,buttons:true},`${language}/${theme}/${width}: ${JSON.stringify(layout)}`);
        const screenshot=await call("Page.captureScreenshot",{format:"png"});
        fs.writeFileSync(path.join(artifacts,`${language}-${theme}-${width}.png`),Buffer.from(screenshot.data,"base64"));
      }
    }
    await evaluate("document.querySelector('[data-log-filter=ERROR]').click(); document.querySelector('.log-file-export').click()");
    await waitFor("!document.querySelector('.log-file-export').disabled");
    const exported = await evaluate("window.__logFixture.exportText");
    assert(exported.includes("[INFO]")&&exported.includes("[WARN]")&&exported.includes("[ERROR]"));
    assert(exported.includes("entry-899"), "the export must include entries beyond the displayed rows");
    assert.equal(await evaluate("document.querySelector('.log-file-feedback').textContent"),locales[language].user_log_report.exported);

    await evaluate("window.__logFixture.failExport=true;document.querySelector('.log-file-export').click()");
    await waitFor("!document.querySelector('.log-file-export').disabled");
    assert.equal(await evaluate("document.querySelector('.log-file-feedback').textContent"),locales[language].user_log_report.errors.write_failed);
    await evaluate("window.__logFixture.failExport=false;window.__logFixture.cancelExport=true;document.querySelector('.log-file-export').click()");
    await waitFor("!document.querySelector('.log-file-export').disabled");
    assert.equal(await evaluate("document.querySelector('.log-file-feedback').textContent"), "", "cancel must not report success");

    await evaluate("window.__logFixture.failRead=true;document.dispatchEvent(new Event('visibilitychange'))");
    await waitFor("document.querySelector('.log-file-status').dataset.state==='error'");
    assert.equal(await evaluate("document.querySelector('.log-file-status').textContent"),locales[language].user_log_report.load_error);
    await evaluate("window.__logFixture.failRead=false;window.__unchangedRow=document.querySelector('.log-file-entry');document.dispatchEvent(new Event('visibilitychange'))");
    await waitFor("document.querySelector('.log-file-status').dataset.state==='ready'");
    assert.equal(await evaluate("window.__unchangedRow===document.querySelector('.log-file-entry')"),true,"unchanged snapshots must not rebuild the viewer");
    await evaluate("window.__logFixture.entries=[];window.__logFixture.revision='empty';document.dispatchEvent(new Event('visibilitychange'))");
    await waitFor("!!document.querySelector('.log-file-empty')");
    assert.equal(await evaluate("document.querySelector('.log-file-empty').textContent"),locales[language].user_log_report.empty);
    await evaluate("document.querySelector('.log-file-dialog').close()");
    await waitFor("!document.querySelector('.log-file-dialog')");
    await evaluate("window.__logFixture.failRead=true;window.openUserLogsModal()");
    assert.equal(await evaluate("document.querySelector('.log-file-status').dataset.state"),"error");
    await evaluate("document.querySelector('.log-file-dialog').close()");
    assert.deepEqual(exceptions,[],"uncaught browser errors");
    await call("Page.removeScriptToEvaluateOnNewDocument",{identifier});
  }
  console.log("User-log UI checks passed in DE/EN: metadata, local filters, full exports, errors/cancel, incremental rows, unchanged snapshots and dark/light responsive layouts.");
  console.log(`Screenshots: ${artifacts}`);
  }
} finally {
  socket?.close();
  browser?.kill();
  await new Promise((resolve)=>server.close(resolve));
}
