import fs from 'node:fs/promises';
import path from 'node:path';

export const dmgVolumeName = 'Creditia GPUI';
export const firstLaunchGuideName = '首次打开说明 - First Launch.txt';
export const dmgWindow = { left: 144, top: 120, width: 760, height: 520, iconSize: 64, textSize: 12 };
export const dmgIconPositions = {
  'Creditia.app': [196, 102],
  'Applications': [564, 102],
  'INSTALL.md': [184, 235],
  [firstLaunchGuideName]: [380, 235],
  'package-info.json': [576, 235],
};

const backgroundDirectory = '.background';
const backgroundFile = 'background.png';

export function dmgLayoutAppleScript() {
  const { left, top, width, height, iconSize, textSize } = dmgWindow;
  const icons = Object.entries(dmgIconPositions).map(([name, point]) =>
    `        set position of item "${name}" of installerWindow to {${point.join(', ')}}`
  ).join('\n');

  return `on run argv
  set mountPath to item 1 of argv
  set mountFolder to POSIX file mountPath as alias
  set pictureFile to POSIX file (mountPath & "/${backgroundDirectory}/${backgroundFile}") as alias
  with timeout of 10 seconds
    tell application "Finder"
      open mountFolder
      set installerWindow to container window of (item mountFolder)
      try
        set current view of installerWindow to icon view
        set toolbar visible of installerWindow to false
        set statusbar visible of installerWindow to false
        set bounds of installerWindow to {${left}, ${top}, ${left + width}, ${top + height}}
        tell icon view options of installerWindow
          set arrangement to not arranged
          set icon size to ${iconSize}
          set text size to ${textSize}
          set label position to bottom
          set shows icon preview to false
          set background picture to pictureFile
        end tell
${icons}
        update mountFolder without registering applications
      on error messageText number messageNumber
        close installerWindow
        error messageText number messageNumber
      end try
      close installerWindow
    end tell
  end timeout
end run
`;
}

function uint32(...values) {
  const data = Buffer.alloc(4 * values.length);
  values.forEach((value, index) => data.writeUInt32BE(value, index * 4));
  return data;
}

function aliasTag(tag, value) {
  const data = Buffer.alloc(4 + value.length + (value.length % 2));
  data.writeUInt16BE(tag);
  data.writeUInt16BE(value.length, 2);
  value.copy(data, 4);
  return data;
}

async function backgroundAlias(mount) {
  const directory = path.join(mount, backgroundDirectory);
  const [volume, folder, file] = await Promise.all([
    fs.stat(mount), fs.stat(directory), fs.stat(path.join(directory, backgroundFile)),
  ]);
  // Finder expects a version 2 Carbon alias in icvp. Use the mounted HFS+
  // volume's creation date and file IDs so it survives compression and remounting.
  const secondsSince1904 = stats => Math.floor(stats.birthtimeMs / 1000) + 2082844800;
  const record = Buffer.alloc(150);
  record.writeUInt16BE(2, 6);
  record[10] = record.write(dmgVolumeName, 11, 'ascii');
  record.writeUInt32BE(secondsSince1904(volume), 38);
  record.write('H+', 42, 'ascii');
  record.writeUInt32BE(folder.ino, 46);
  record[50] = record.write(backgroundFile, 51, 'ascii');
  record.writeUInt32BE(file.ino, 114);
  record.writeUInt32BE(secondsSince1904(file), 118);
  record.writeInt16BE(-1, 130);
  record.writeInt16BE(-1, 132);

  const tags = Buffer.concat([
    aliasTag(0, Buffer.from(backgroundDirectory)),
    aliasTag(1, uint32(folder.ino)),
    aliasTag(2, Buffer.from(`${dmgVolumeName}:${backgroundDirectory}:\0${backgroundFile}`)),
    aliasTag(18, Buffer.from(`/${backgroundDirectory}/${backgroundFile}`)),
    aliasTag(19, Buffer.from(`/Volumes/${dmgVolumeName}`)),
    aliasTag(0xffff, Buffer.alloc(0)),
  ]);
  record.writeUInt16BE(record.length + tags.length, 4);
  return Buffer.concat([record, tags]);
}

function finderRecord(name, code, type, value) {
  const filename = Buffer.from(name.normalize('NFD'), 'utf16le').swap16();
  const key = Buffer.concat([uint32(filename.length / 2), filename, Buffer.from(code + type)]);
  return type === 'blob' ? Buffer.concat([key, uint32(value.length), value]) : Buffer.concat([key, value]);
}

function finderStore(records) {
  // The fixed set of Finder records fits in one B-tree leaf.
  // https://metacpan.org/dist/Mac-Finder-DSStore/view/DSStoreFormat.pod
  const directory = 2048, tree = 32, leaf = 4096, pageSize = 4096;
  const file = Buffer.alloc(4 + leaf + pageSize);
  uint32(1).copy(file);
  file.write('Bud1', 4, 'ascii');
  uint32(directory, 2048, directory).copy(file, 8);
  uint32(0x100c, 0x87, 0x200b, 0).copy(file, 20);

  const blockTable = Buffer.alloc(256 * 4);
  uint32(directory | 11, tree | 5, leaf | 12).copy(blockTable);
  const free = Array.from({ length: 32 }, (_, order) =>
    order <= 5 || [11, 12, 31].includes(order) ? uint32(0) : uint32(1, 2 ** order)
  );
  Buffer.concat([
    uint32(3, 0), blockTable, uint32(1), Buffer.from('\x04DSDB'), uint32(1), ...free,
  ]).copy(file, 4 + directory);
  uint32(2, 1, records.length, 1, pageSize).copy(file, 4 + tree);
  const page = Buffer.concat([uint32(0, records.length), ...records]);
  if (page.length > pageSize) throw new Error('DMG Finder records exceed one page');
  page.copy(file, 4 + leaf);
  return file;
}

function binaryPlist(run, fields) {
  return run('plutil', ['-convert', 'binary1', '-o', '-', '-'], {
    input: `<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict>${fields}</dict></plist>`,
    stdio: ['pipe', 'pipe', 'inherit'],
  });
}

async function finderMetadata(mount, run) {
  const { left, top, width, height, iconSize, textSize } = dmgWindow;
  const window = binaryPlist(run, `
    <key>WindowBounds</key><string>{{${left}, ${top}}, {${width}, ${height}}}</string>
    <key>ShowToolbar</key><false/>
    <key>ShowSidebar</key><false/>
    <key>ContainerShowSidebar</key><false/>
    <key>ShowStatusBar</key><false/>
    <key>ShowPathbar</key><false/>
    <key>ShowTabView</key><false/>
  `);
  const alias = await backgroundAlias(mount);
  const iconView = binaryPlist(run, `
    <key>viewOptionsVersion</key><integer>1</integer>
    <key>backgroundType</key><integer>2</integer>
    <key>backgroundImageAlias</key><data>${alias.toString('base64')}</data>
    <key>arrangeBy</key><string>none</string>
    <key>iconSize</key><real>${iconSize}</real>
    <key>textSize</key><real>${textSize}</real>
    <key>labelOnBottom</key><true/>
    <key>showIconPreview</key><false/>
    <key>showItemInfo</key><false/>
    <key>scrollPositionX</key><real>0</real>
    <key>scrollPositionY</key><real>0</real>
  `);
  const locations = Object.keys(dmgIconPositions).sort((a, b) => {
    const left = a.toLowerCase(), right = b.toLowerCase();
    return left < right ? -1 : left > right ? 1 : 0;
  }).map(name => finderRecord(name, 'Iloc', 'blob', uint32(...dmgIconPositions[name], 0xffffffff, 0xffff0000)));
  return finderStore([
    finderRecord('.', 'bwsp', 'blob', window),
    finderRecord('.', 'icvl', 'type', Buffer.from('icnv')),
    finderRecord('.', 'icvp', 'blob', iconView),
    finderRecord('.', 'vSrn', 'long', uint32(1)),
    ...locations,
  ]);
}

export function tryFinderLayout(mount, run) {
  try {
    run('osascript', ['-', mount], {
      input: dmgLayoutAppleScript(),
      stdio: 'pipe',
      timeout: 15_000,
      killSignal: 'SIGKILL',
    });
    return true;
  } catch (error) {
    console.warn(`Finder layout unavailable (${error.code ?? error.status}); using saved DMG layout.`);
    return false;
  }
}

export async function applyDmgLayout(mount, run) {
  const store = path.join(mount, '.DS_Store');
  const metadata = await finderMetadata(mount, run);
  await fs.writeFile(store, metadata);
  // CI uses the on-disk layout directly. Finder may be absent or unable to
  // accept Apple events.
  if (!process.env.CI && !tryFinderLayout(mount, run)) {
    await fs.writeFile(store, metadata);
  }
}
