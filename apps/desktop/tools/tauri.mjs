import { createRequire } from 'node:module';
import path from 'node:path';

// Keep actool's stdin open under the Node CLI until the upstream fix ships:
// https://github.com/tauri-apps/tauri/pull/15991
if (process.platform === 'darwin') {
  process.env.PATH = `${path.join(import.meta.dirname, 'macos')}${path.delimiter}${process.env.PATH ?? ''}`;
}
createRequire(import.meta.url)('@tauri-apps/cli/tauri.js');
