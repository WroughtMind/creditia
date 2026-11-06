const { test } = require('node:test');
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const { resolve } = require('node:path');
const vm = require('node:vm');
const source = readFileSync(resolve(__dirname, '../src-tauri/src/migration.js'), 'utf8');

function context(entries, reject = false) {
  const values = new Map(Object.entries(entries)), calls = [];
  const sandbox = {
    localStorage: { get length() { return values.size; }, key: index => [...values.keys()][index] ?? null, getItem: key => values.get(key) ?? null },
    console: { error() {} },
    window: { __TAURI__: { core: { invoke: async (command, args) => {
      calls.push({ command, args });
      if (reject && command === 'import_legacy_preferences') throw new Error('disk unavailable');
      return {};
    } } } },
  };
  return { sandbox: vm.createContext(sandbox), calls, values };
}

test('minimal helper imports privacy, project groups and views without starting UI', async () => {
  const test = context({ 'creditia.names': 'off', 'creditia.group.p': 'model', 'creditia.theme': 'ink', 'creditia.view': 'roam', 'creditia.dims': '{}', 'unrelated': 'private' });
  await vm.runInContext(source, test.sandbox);
  assert.deepEqual(JSON.parse(JSON.stringify(test.calls)), [
    { command: 'import_legacy_preferences', args: { values: { 'creditia.names': 'off', 'creditia.group.p': 'model', 'creditia.theme': 'ink', 'creditia.view': 'roam' } } },
    { command: 'migration_done', args: { success: true } },
  ]);
  assert.equal(test.values.get('creditia.names'), 'off', 'the original WebView storage remains available for rollback');
  await vm.runInContext(source, test.sandbox);
  assert.equal(test.calls.length, 2, 'repeated page callbacks cannot import twice');
});

test('failed preference import exits unsuccessful and preserves old state', async () => {
  const test = context({ 'creditia.names': 'off' }, true);
  await vm.runInContext(source, test.sandbox);
  assert.equal(test.calls.at(-1).command, 'migration_done');
  assert.equal(test.calls.at(-1).args.success, false);
  assert.equal(test.values.get('creditia.names'), 'off');
});
