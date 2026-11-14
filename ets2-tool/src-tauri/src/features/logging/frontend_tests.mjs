import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const source = await readFile(new URL("../../../../src/modules/shared/diagnostics_runtime.js", import.meta.url), "utf8");
const listeners = new Map();
let failTransport = false;
let calls = [];
const original = new Error("backup permission denied");
const raw = async (command, args) => {
  calls.push({ command, args });
  if (command === "diagnostics_record") {
    if (failTransport) throw new Error("disk full");
    return `ERR-test-${calls.length}`;
  }
  if (command === "test_failed_command") throw original;
  return { private_result: "must not be recorded" };
};
globalThis.window = {
  __TAURI__: { core: { invoke: raw } },
  addEventListener: (name, handler) => { const current = listeners.get(name) || []; current.push(handler); listeners.set(name, current); },
};
globalThis.document = {
  body: null, readyState: "loading", head: { append() {} },
  createElement: () => ({}), addEventListener() {},
};
const runtime = await import(`data:text/javascript;base64,${Buffer.from(source).toString("base64")}`);

test("IPC events share an operation ID and exclude raw arguments and results", async () => {
  calls = [];
  const result = await runtime.diagnosticInvoke("save_test_success", { password: "private_password", profilePath: "C:/Users/Private/profile" });
  assert.equal(result.private_result, "must not be recorded");
  const entries = calls.filter((call) => call.command === "diagnostics_record").map((call) => call.args.event);
  assert.equal(entries.length, 2);
  assert.equal(entries[0].operationId, entries[1].operationId);
  const encoded = JSON.stringify(entries);
  assert(!encoded.includes("private_password")); assert(!encoded.includes("Users/Private")); assert(!encoded.includes("private_result"));
});

test("failed commands preserve the original rejection and record the cause", async () => {
  calls = [];
  await assert.rejects(runtime.diagnosticInvoke("test_failed_command"), (error) => error === original);
  const event = calls.filter((call) => call.command === "diagnostics_record").at(-1).args.event;
  assert.equal(event.level, "ERROR"); assert.equal(event.technical, original.message); assert(event.stack.includes("backup permission denied"));
});

test("diagnostic transport failure does not change application results", async () => {
  failTransport = true;
  assert.equal((await runtime.diagnosticInvoke("save_test_success")).private_result, "must not be recorded");
  await assert.rejects(runtime.diagnosticInvoke("test_failed_command"), (error) => error === original);
  failTransport = false;
});

test("internal diagnostic commands do not recursively log themselves", async () => {
  calls = [];
  await runtime.diagnosticInvoke("diagnostics_events");
  assert.equal(calls.length, 1);
});

test("log polling and report export do not add recursive log entries", async () => {
  calls = [];
  for (const command of ["diagnostics_user_logs", "get_log_preview", "export_log_file"]) {
    await runtime.diagnosticInvoke(command);
  }
  assert.equal(calls.length, 3);
  assert.equal(calls.filter((call) => call.command === "diagnostics_record").length, 0);
});

test("global handlers record JavaScript exceptions and unhandled rejections", async () => {
  calls = [];
  listeners.get("error")[0]({ error: new Error("JS test exception"), filename: "/main.js", lineno: 12, colno: 2 });
  listeners.get("unhandledrejection")[0]({ reason: new Error("promise test rejection") });
  await new Promise((resolve) => setImmediate(resolve));
  const actions = calls.filter((call) => call.command === "diagnostics_record").map((call) => call.args.event.action);
  assert(actions.includes("exception")); assert(actions.includes("unhandled_rejection"));
});

test("global handler installation is idempotent", async () => {
  await import(`data:text/javascript;base64,${Buffer.from(`${source}\n// second evaluation`).toString("base64")}`);
  assert.equal(listeners.get("error").length, 1); assert.equal(listeners.get("unhandledrejection").length, 1);
});

test("nested JavaScript causes are retained without serializing arbitrary objects", async () => {
  calls = [];
  listeners.get("unhandledrejection")[0]({ reason: new Error("outer error", { cause: new Error("inner cause") }) });
  await new Promise((resolve) => setImmediate(resolve));
  const event = calls.find((call) => call.command === "diagnostics_record").args.event;
  assert.deepEqual(event.causes, ["inner cause"]);
});

test("a rejected IPC error is not logged again as an unhandled rejection", async () => {
  calls = [];
  await assert.rejects(runtime.diagnosticInvoke("test_failed_command"));
  const before = calls.filter((call) => call.command === "diagnostics_record").length;
  listeners.get("unhandledrejection")[0]({ reason: original });
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(calls.filter((call) => call.command === "diagnostics_record").length, before);
});

test("repeated global errors are bounded and ordinary reads do not flood the log", async () => {
  calls = [];
  for (let i = 0; i < 500; i += 1) listeners.get("unhandledrejection")[0]({ reason: "repeating rejection" });
  await runtime.diagnosticInvoke("get_test_data");
  assert.equal(calls.filter((call) => call.command === "diagnostics_record").length, 1);
  assert(window.__ETS2_DIAGNOSTICS__.events.size <= 128);
});

test("console failures are captured without serializing arbitrary objects", async () => {
  calls = [];
  console.error("handled JS failure", { token: "private-token-object" });
  await new Promise((resolve) => setImmediate(resolve));
  const entries = calls.filter((call) => call.command === "diagnostics_record");
  assert.equal(entries.length, 1);
  assert.equal(entries[0].args.event.action, "console_error");
  assert(!JSON.stringify(entries).includes("private-token-object"));
});

test("known user/configuration errors are not classified as fatal bugs", () => {
  assert(runtime.isExpectedError("load_profile", "no_profile_selected"));
  assert(runtime.isExpectedError("get_player_trailer", "active_trailer_not_found"));
  assert(!runtime.isExpectedError("load_profile", "unexpected parser failure"));
});
