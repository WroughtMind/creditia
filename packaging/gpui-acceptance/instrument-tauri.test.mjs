import assert from 'node:assert/strict';
import test from 'node:test';
import { instrumentSources } from './instrument-tauri.mjs';
const frontend = `async function api(cmd: string, args: any = {}): Promise<any> {\n  if (native) return native(cmd,args);\n}\nfunction paint() { /* unchanged production renderer */ }\nboot();\n`;
const rust = `fn main() {\n tauri::Builder::default().setup(|app| { original_background_service(); Ok(()) })\n .invoke_handler(tauri::generate_handler![projects, credits, preview::render_preview]);\n}\n`;
test('baseline overlay preserves production render and input code and has an explicit runtime guard', () => {
  const [js, native] = instrumentSources([frontend, rust]);
  assert.ok(js.includes('function paint() { /* unchanged production renderer */ }'));
  assert.ok(native.includes('projects, credits, preview::render_preview'));
  assert.equal([...js.matchAll(/void creditiaAcceptanceStart\(\);/g)].length, 1);
  assert.ok(native.includes('if creditia_acceptance_enabled() { return Ok(()); }'));
  assert.ok(native.includes('CREDITIA_ACCEPTANCE_MODE'));
});
test('missing, duplicate or already patched anchors fail instead of silently changing benchmark scope', () => {
  assert.throws(() => instrumentSources([frontend.replace('boot();', ''), rust]), /startup anchor/);
  assert.throws(() => instrumentSources([frontend + '\nboot();', rust]), /startup anchor/);
  assert.throws(() => instrumentSources(instrumentSources([frontend, rust])), /already instrumented/);
});
