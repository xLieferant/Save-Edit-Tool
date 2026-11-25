import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import test from "node:test";

const source = fs.readFileSync(new URL("../../../../src/modules/shared/diagnostics_runtime.js", import.meta.url), "utf8");

function runtime(invoke) {
  const toasts = [];
  const events = [];
  const calls = [];
  const window = {
    __TAURI__: { core: { invoke: async (command, args) => {
      calls.push([command, args]);
      return command === "diagnostics_record" ? null : invoke(command, args);
    } } },
    saveSync: { mutations: 0, accepting: false, blockWrites: false },
    showToast: (...args) => toasts.push(args),
    addEventListener() {},
    dispatchEvent: (event) => events.push(event),
  };
  const context = { window, performance, crypto: globalThis.crypto,
    console: { error() {}, warn() {} },
    CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
  };
  vm.runInNewContext(source.replaceAll("export ", ""), context);
  return { ...context, toasts, events, calls };
}

test("save success waits for completion and does not apply to global settings", async () => {
  let complete;
  const app = runtime(() => new Promise((resolve) => { complete = resolve; }));
  const pending = app.diagnosticInvoke("edit_player_money", { value: 12345 });
  assert.equal(app.toasts.length, 0);
  assert.equal(app.window.saveSync.mutations, 1);
  complete(null);
  await pending;
  assert.equal(app.toasts[0][0], "save_errors.success");
  assert.equal(app.events[0].type, "savegame-modified");
  assert.equal(app.window.saveSync.mutations, 0);
  const configuration = app.diagnosticInvoke("edit_config_value", {});
  complete(null);
  await configuration;
  assert.equal(app.toasts.length, 1);
});

test("failed writes report localized causes and never report success", async () => {
  for (const [error, expected] of [
    ["save_changed_since_load", "save_errors.changed"],
    ["save_errors.locked", "save_errors.locked"],
    ["decode_failed | source=fixture", "save_errors.decrypt_failed"],
    ["save_post_write_verification_failed", "save_errors.verification_failed"],
    ["save_errors.backup_failed", "save_errors.backup_failed"],
    ["game_sii_not_found", "save_errors.not_found"],
  ]) {
    const app = runtime(async () => { throw error; });
    await assert.rejects(app.diagnosticInvoke("edit_player_money", { value: 12345 }), (result) => result === expected);
    assert.equal(app.toasts.length, 1);
    assert.equal(app.toasts[0][0], expected);
    assert.equal(app.toasts[0][2], "error");
    assert.equal(app.events.length, 0);
    assert.equal(app.window.saveSync.mutations, 0);
    assert.ok(app.calls.some(([command, args]) => command === "diagnostics_record" && args.event.technical === error));
  }
});

test("all IPC adapters respect a draft conflict before starting a write", async () => {
  const app = runtime(async () => null);
  app.window.saveSync.blockWrites = true;
  await assert.rejects(app.diagnosticInvoke("apply_active_trailer_switch", {}), (error) => error === "save_sync.conflict");
  assert.equal(app.calls.some(([command]) => command === "apply_active_trailer_switch"), false);
  assert.equal(app.window.saveSync.mutations, 0);
  assert.equal(app.events.length, 0);
});

test("refresh can wait for an editor readback without misclassifying it as a write", async () => {
  let complete;
  const app = runtime(() => new Promise((resolve) => { complete = resolve; }));
  const pending = app.diagnosticInvoke("read_all_save_data", {});
  assert.equal(app.window.saveSync.reads, 1);
  assert.equal(app.window.saveSync.mutations, 0);
  complete({ money: 12345 });
  await pending;
  assert.equal(app.window.saveSync.reads, 0);
  assert.equal(app.toasts.length, 0);
});
