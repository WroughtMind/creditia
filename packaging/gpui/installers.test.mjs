import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { crc32, inflateSync } from 'node:zlib';
import { dmgIconPositions, dmgWindow, firstLaunchGuideName, tryFinderLayout } from './dmg-layout.mjs';
import { dmgLayoutAppleScript, firstLaunchGuideText, nsisScript } from './installers.mjs';
import { seal } from './package.mjs';

const guideCommand = 'xattr -dr com.apple.quarantine /Applications/Creditia.app';
const background = new URL('./dmg-background.png', import.meta.url);
const installNotes = new URL('./INSTALL.md', import.meta.url);
const installerScript = fileURLToPath(new URL('./installers.mjs', import.meta.url));
const detachScript = fileURLToPath(new URL('../detach-dmg.sh', import.meta.url));

test('the first-launch guide and install notes give the same bilingual steps and command', async () => {
  for (const text of [firstLaunchGuideText(), await fs.readFile(installNotes, 'utf8')]) {
    for (const phrase of ['Privacy & Security', '隐私与安全性', 'Open Anyway', '仍要打开']) {
      assert.ok(text.includes(phrase), `Missing ${phrase}`);
    }
    assert.ok(text.split('\n').some(line => line.trim() === guideCommand));
  }
  assert.ok(firstLaunchGuideText().split('\n').includes(guideCommand));
});

test('the DMG background is a valid PNG matching the Finder window bounds', async () => {
  const png = await fs.readFile(background);
  assert.equal(png.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  assert.equal(png.toString('ascii', 12, 16), 'IHDR');
  const width = png.readUInt32BE(16), height = png.readUInt32BE(20);
  assert.deepEqual([width, height], [720, 480]);
  assert.deepEqual([width, height], [dmgWindow.width, dmgWindow.height]);
  assert.deepEqual([...png.subarray(24, 29)], [8, 6, 0, 0, 0]);
  const data = [];
  let offset = 8;
  for (; offset < png.length;) {
    const length = png.readUInt32BE(offset);
    const end = offset + 8 + length;
    assert.ok(end + 4 <= png.length);
    assert.equal(crc32(png.subarray(offset + 4, end)), png.readUInt32BE(end));
    if (png.toString('ascii', offset + 4, offset + 8) === 'IDAT') data.push(png.subarray(offset + 8, end));
    offset = end + 4;
  }
  assert.equal(png.toString('ascii', offset - 8, offset - 4), 'IEND');
  assert.equal(inflateSync(Buffer.concat(data)).length, height * (1 + width * 4));
});

test('NSIS adds bilingual welcome and finish pages in order and preserves NSIS escaping', () => {
  const script = nsisScript({
    root: String.raw`C:\Build $ Work\候选`,
    exe: String.raw`C:\Build $ Work\Creditia setup.exe`,
    version: '0.1.0-$preview',
    commit: '6422ae914f',
  });
  assert.deepEqual([...script.matchAll(/^!insertmacro (MUI_PAGE_\w+)$/gm)].map(match => match[1]), [
    'MUI_PAGE_WELCOME', 'MUI_PAGE_DIRECTORY', 'MUI_PAGE_INSTFILES', 'MUI_PAGE_FINISH',
  ]);
  assert.match(script, /SmartScreen.*Windows protected your PC.*More info → Run anyway/);
  assert.match(script, /更多信息.*仍要运行/);
  assert.match(script, /Keep resources and creditia-core\.exe together/);
  assert.match(script, /MUI_FINISHPAGE_TEXT.*安装已完成/);
  assert.ok(script.includes(String.raw`!define MUI_FINISHPAGE_RUN "$INSTDIR\creditia-gpui.exe"`));
  assert.ok(script.includes(String.raw`$\"Windows protected your PC$\"`));
  assert.ok(script.includes(String.raw`$\r$\n`));
  assert.ok(script.includes('File /r "C:\\Build ${U+24} Work\\候选\\*"'));
  assert.ok(script.includes('OutFile "C:\\Build ${U+24} Work\\Creditia setup.exe"'));
  assert.ok(script.includes(String.raw`InstallDir "$LOCALAPPDATA\Programs\Creditia GPUI\0.1.0-$$preview-6422ae91"`));
});

test('Finder AppleScript is bounded and a timeout uses the saved layout', t => {
  const script = dmgLayoutAppleScript();
  const {left, top, width, height} = dmgWindow;
  assert.ok(script.includes(`set bounds of installerWindow to {${left}, ${top}, ${left + width}, ${top + height}}`));
  assert.match(script, /set current view of installerWindow to icon view/);
  assert.match(script, /\.background\/background\.png/);
  for (const [name, position] of Object.entries(dmgIconPositions)) {
    assert.ok(script.includes(`set position of item "${name}" of installerWindow to {${position.join(', ')}}`));
  }
  const mount = '/path with spaces/Creditia "Test"';
  const warning = t.mock.method(console, 'warn', () => {});
  assert.equal(tryFinderLayout(mount, (program, args, settings) => {
    assert.equal(program, 'osascript');
    assert.deepEqual(args, ['-', mount]);
    assert.equal(settings.input, script);
    assert.equal(settings.timeout, 15_000);
    assert.equal(settings.killSignal, 'SIGKILL');
    throw Object.assign(new Error('Finder timed out'), { code: 'ETIMEDOUT' });
  }), false);
  assert.match(warning.mock.calls[0].arguments[0], /ETIMEDOUT.*using saved DMG layout/);
});

function readFinderRecords(store) {
  assert.equal(store.toString('ascii', 4, 8), 'Bud1');
  const blockTable = store.readUInt32BE(8) + 12;
  const blockOffset = index => 4 + (store.readUInt32BE(blockTable + 4 * index) & ~0x1f);
  const tree = blockOffset(1);
  assert.equal(store.readUInt32BE(tree + 12), 1);
  const leaf = blockOffset(store.readUInt32BE(tree));
  assert.equal(store.readUInt32BE(leaf), 0);
  const records = new Map();
  let offset = leaf + 8;
  for (let i = 0; i < store.readUInt32BE(leaf + 4); i++) {
    const length = store.readUInt32BE(offset) * 2;
    const name = Buffer.from(store.subarray(offset + 4, offset + 4 + length)).swap16().toString('utf16le');
    const key = offset + 4 + length;
    const code = store.toString('ascii', key, key + 4);
    const type = store.toString('ascii', key + 4, key + 8);
    offset = key + 8;
    const bytes = type === 'blob' ? store.readUInt32BE(offset) : 4;
    if (type === 'blob') offset += 4;
    records.set(`${name}/${code}`, store.subarray(offset, offset + bytes));
    offset += bytes;
  }
  return records;
}

test('a real macOS DMG keeps the signed payload and Finder layout after an AppleScript failure',
  { skip: process.platform !== 'darwin', timeout: 90_000 }, async () => {
    const temporary = await fs.mkdtemp(path.join(os.tmpdir(), 'creditia-dmg-test-'));
    const root = path.join(temporary, 'creditia-gpui-fixture');
    const installers = path.join(temporary, 'installers');
    const mount = path.join(temporary, 'read-only mount');
    const run = (program, args, settings = {}) =>
      execFileSync(program, args, { encoding: 'utf8', stdio: 'pipe', timeout: 60_000, ...settings });
    try {
      const app = path.join(root, 'Creditia.app');
      const contents = path.join(app, 'Contents');
      await fs.mkdir(path.join(contents, 'MacOS'), { recursive: true });
      await fs.copyFile('/usr/bin/true', path.join(contents, 'MacOS', 'fixture'));
      await fs.writeFile(path.join(contents, 'Info.plist'), `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>fixture</string>
<key>CFBundleIdentifier</key><string>com.creditia.dmg-fixture</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
</dict></plist>
`);
      run('codesign', ['--force', '--sign', '-', app]);
      await fs.copyFile(installNotes, path.join(root, 'INSTALL.md'));
      await fs.writeFile(path.join(root, 'package-info.json'), JSON.stringify({
        schema: 1, target: 'fixture-apple-darwin', platform: 'darwin', version: '0.1.0',
        commit: 'a'.repeat(40), fontFiles: [],
      }));
      await seal(root);

      const bin = path.join(temporary, 'bin');
      await fs.mkdir(bin);
      // Simulate Finder writing partial metadata before denying automation.
      await fs.writeFile(path.join(bin, 'osascript'), '#!/bin/sh\nprintf partial > "$2/.DS_Store"\nprintf called > "$CREDITIA_FINDER_TEST_MARKER"\nexit 1\n', { mode: 0o755 });
      const marker = path.join(bin, 'finder-called');
      const env = { ...process.env, PATH: bin + path.delimiter + process.env.PATH, CREDITIA_FINDER_TEST_MARKER: marker };
      delete env.CI;
      run(process.execPath, [installerScript, '--root', root, '--output', installers], { env });
      assert.equal(await fs.readFile(marker, 'utf8'), 'called');
      const dmg = path.join(installers, path.basename(root) + '.dmg');
      run('hdiutil', ['verify', dmg]);
      await fs.mkdir(mount);
      run('hdiutil', ['attach', '-readonly', '-nobrowse', '-noautoopen', '-mountpoint', mount, dmg]);
      try {
        run('codesign', ['--verify', '--strict', path.join(mount, 'Creditia.app')]);
        assert.equal(await fs.readlink(path.join(mount, 'Applications')), '/Applications');
        assert.equal(await fs.readFile(path.join(mount, firstLaunchGuideName), 'utf8'), firstLaunchGuideText());
        for (const name of ['INSTALL.md', 'package-info.json']) {
          assert.deepEqual(await fs.readFile(path.join(mount, name)), await fs.readFile(path.join(root, name)));
        }
        const picture = path.join(mount, '.background', 'background.png');
        assert.deepEqual(await fs.readFile(picture), await fs.readFile(background));

        const records = readFinderRecords(await fs.readFile(path.join(mount, '.DS_Store')));
        assert.equal(records.get('./icvl').toString(), 'icnv');
        const plist = (code, key) => run('plutil', ['-extract', key, 'raw', '-o', '-', '-'], { input: records.get(`./${code}`) }).trim();
        const {left, top, width, height} = dmgWindow;
        assert.equal(plist('bwsp', 'WindowBounds'), `{{${left}, ${top}}, {${width}, ${height}}}`);
        assert.equal(plist('icvp', 'backgroundType'), '2');
        const alias = Buffer.from(plist('icvp', 'backgroundImageAlias'), 'base64');
        assert.equal(alias.readUInt32BE(114), (await fs.stat(picture)).ino);
        for (const [name, position] of Object.entries(dmgIconPositions)) {
          const record = records.get(`${name}/Iloc`);
          assert.deepEqual([record.readUInt32BE(0), record.readUInt32BE(4)], position);
        }
      } finally {
        run('bash', [detachScript, mount]);
      }
      assert.deepEqual((await fs.readdir(installers)).sort(), [
        'SHA256SUMS', 'candidate.json', 'creditia-gpui-fixture.dmg', 'creditia-gpui-fixture.zip',
      ]);
    } finally {
      // Do not recursively clean up the directory if the mounted image is busy.
      await fs.rmdir(mount).catch(error => { if (error.code !== 'ENOENT') throw error; });
      await fs.rm(temporary, { recursive: true, force: true });
    }
  }
);
