#!/usr/bin/env node
// Archive and native installer generation is separate from staging/sealing.
import fs from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { applyDmgLayout, dmgVolumeName, firstLaunchGuideName } from './dmg-layout.mjs';
import { verify } from './package.mjs';

export { dmgLayoutAppleScript } from './dmg-layout.mjs';

export const firstLaunchCommand = 'xattr -dr com.apple.quarantine /Applications/Creditia.app';
const background = new URL('./dmg-background.png', import.meta.url);
const detachScript = fileURLToPath(new URL('../detach-dmg.sh', import.meta.url));
const run = (program, args, settings = {}) => execFileSync(program, args, { stdio: 'inherit', ...settings });

export function firstLaunchGuideText() {
  return [
    '首次打开 Creditia',
    '',
    '1. 将 Creditia.app 拖入「应用程序」，然后从「应用程序」打开 Creditia。',
    '2. 此版本未经 Apple 公证。若已核对下载来源与校验值，且 macOS 提示无法验证或已损坏，先尝试打开一次。',
    '3. 打开「系统设置 → 隐私与安全性」，在「安全性」中点击「仍要打开」，然后确认。',
    '   也可以在「终端」粘贴并运行下方命令，再从「应用程序」打开 Creditia。',
    '',
    firstLaunchCommand,
    '',
    'First launch of Creditia',
    '',
    '1. Drag Creditia.app into Applications, then open it from Applications.',
    '2. This build has not been notarized by Apple. If macOS blocks first launch, verify the download and try opening Creditia once.',
    '3. In System Settings → Privacy & Security, find the Security section, click Open Anyway, and confirm.',
    '   You can also paste the command below into Terminal, then open Creditia from Applications.',
    '',
    firstLaunchCommand,
    '',
  ].join('\n');
}

function escapeNsis(value) {
  return value.replaceAll('$', () => '$$').replaceAll('"', '$\\"');
}

function escapeNsisPath(value) {
  // File and OutFile read compiler paths before NSIS decodes runtime strings.
  return value.replaceAll('$', '${U+24}').replaceAll('"', '$\\"');
}

export function nsisScript({ root, exe, version, commit }) {
  const revision = escapeNsis(version + '-' + (commit ?? 'local').slice(0, 8));
  const welcome = [
    'This installer is unsigned. If SmartScreen shows "Windows protected your PC", choose More info → Run anyway.',
    'Keep resources and creditia-core.exe together in the install folder.',
    '',
    '此安装程序未签名。若 SmartScreen 提示「Windows 已保护你的电脑」，请选择「更多信息」→「仍要运行」。',
    '请将 resources 文件夹和 creditia-core.exe 保留在同一安装目录。',
    '',
    'Click Next to continue. / 点击「下一步」继续。',
  ].map(escapeNsis).join('$\\r$\\n');
  const finish = [
    'Creditia is installed. Keep the install folder together if you move it.',
    '',
    '安装已完成。移动 Creditia 时，请保留完整安装目录。',
  ].map(escapeNsis).join('$\\r$\\n');

  return String.raw`Unicode true
!include "MUI2.nsh"
Name "Creditia GPUI ${revision}"
OutFile "${escapeNsisPath(exe)}"
InstallDir "$LOCALAPPDATA\Programs\Creditia GPUI${'\\' + revision}"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_WELCOMEPAGE_TITLE "Install Creditia / 安装 Creditia"
!define MUI_WELCOMEPAGE_TEXT "${welcome}"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TEXT "${finish}"
!define MUI_FINISHPAGE_RUN "$INSTDIR\creditia-gpui.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch Creditia / 打开 Creditia"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Install"
SetOutPath "$INSTDIR"
File /r "${escapeNsisPath(root)}\*"
CreateDirectory "$SMPROGRAMS\Creditia GPUI"
CreateShortCut "$SMPROGRAMS\Creditia GPUI\Creditia ${revision}.lnk" "$INSTDIR\creditia-gpui.exe"
WriteUninstaller "$INSTDIR\uninstall.exe"
WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CreditiaGPUI-${revision}" "DisplayName" "Creditia GPUI ${revision}"
WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CreditiaGPUI-${revision}" "UninstallString" '"$INSTDIR\uninstall.exe"'
WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CreditiaGPUI-${revision}" "DisplayVersion" "${escapeNsis(version)}"
SectionEnd

Section "Uninstall"
Delete "$SMPROGRAMS\Creditia GPUI\Creditia ${revision}.lnk"
RMDir "$SMPROGRAMS\Creditia GPUI"
DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CreditiaGPUI-${revision}"
RMDir /r "$INSTDIR"
SectionEnd
`;
}

function detachDmg(mount) {
  try {
    run('bash', [detachScript, mount]);
  } catch (error) {
    if (error.status !== 16) throw error;
    run('hdiutil', ['detach', mount, '-force']);
  }
}

async function buildDmg(root, output, name) {
  const work = await fs.mkdtemp(path.join(output, '.dmg-'));
  const source = path.join(work, 'source');
  const mount = path.join(work, 'mount');
  const writable = path.join(work, 'writable.dmg');
  const compressed = path.join(work, name + '.dmg');
  try {
    await fs.mkdir(source);
    run('ditto', [path.join(root, 'Creditia.app'), path.join(source, 'Creditia.app')]);
    for (const file of ['INSTALL.md', 'package-info.json']) {
      await fs.copyFile(path.join(root, file), path.join(source, file));
    }
    await fs.writeFile(path.join(source, firstLaunchGuideName), firstLaunchGuideText());
    await fs.symlink('/Applications', path.join(source, 'Applications'));
    run('hdiutil', ['create', '-volname', dmgVolumeName, '-fs', 'HFS+', '-srcfolder', source, '-format', 'UDRW', writable]);

    // A source-folder image is sized for the existing payload. Reserve room for
    // the artwork and Finder metadata that are written to the mounted volume.
    const bytes = (await fs.stat(writable)).size + (await fs.stat(background)).size + 1024 * 1024;
    run('hdiutil', ['resize', '-sectors', String(Math.ceil(bytes / 512)), writable]);
    await fs.mkdir(mount);
    run('hdiutil', ['attach', '-nobrowse', '-noautoopen', '-mountpoint', mount, writable]);
    try {
      await fs.mkdir(path.join(mount, '.background'));
      await fs.copyFile(background, path.join(mount, '.background', 'background.png'));
      await applyDmgLayout(mount, run);
    } finally {
      detachDmg(mount);
    }
    run('hdiutil', ['convert', writable, '-format', 'UDZO', '-o', compressed]);
    run('hdiutil', ['verify', compressed]);
    await fs.rename(compressed, path.join(output, path.basename(compressed)));
  } finally {
    // Never recursively remove the work directory while a volume is mounted.
    await fs.rmdir(mount).catch(error => { if (error.code !== 'ENOENT') throw error; });
    await fs.rm(work, { recursive: true, force: true });
  }
}

async function main(args) {
  const options = Object.fromEntries(args.reduce((pairs, arg, i, all) => i % 2 ? pairs : [...pairs, [arg.replace(/^--/, ''), all[i + 1]]], []));
  if (!options.root || !options.output) throw new Error('Usage: installers.mjs --root package-directory --output installer-directory');
  const root = path.resolve(options.root), output = path.resolve(options.output);
  const info = await verify(root), name = path.basename(root);
  await fs.mkdir(output, { recursive: true });
  const notes = { target: info.target, candidate_sha: info.commit, installers: [], signing: info.platform === 'darwin' ? 'ad-hoc macOS signature; not notarized' : 'unsigned candidate' };

  if (info.platform === 'darwin') {
    const archive = path.join(output, name + '.zip');
    run('ditto', ['-c', '-k', '--sequesterRsrc', '--keepParent', root, archive]);
    notes.installers.push(path.basename(archive));
    await buildDmg(root, output, name);
    notes.installers.push(name + '.dmg');
  } else if (info.platform === 'win32') {
    const archive = path.join(output, name + '.zip');
    const tar = path.join(process.env.SystemRoot, 'System32', 'tar.exe');
    run(tar, ['-a', '-c', '-f', archive, '-C', path.dirname(root), name]);
    notes.installers.push(path.basename(archive));
    const nsis = process.env.MAKENSIS ?? 'C:\\Program Files (x86)\\NSIS\\makensis.exe';
    await fs.access(nsis);
    const exe = path.join(output, name + '-setup.exe');
    const source = path.join(output, 'installer.nsi');
    try {
      // A UTF-8 BOM makes NSIS read the bilingual text independently of the system code page.
      await fs.writeFile(source, '\uFEFF' + nsisScript({ root, exe, version: info.version, commit: info.commit }));
      run(nsis, ['/V2', source]);
    } finally {
      await fs.rm(source, { force: true });
    }
    notes.installers.push(path.basename(exe));
  } else {
    const archive = path.join(output,`${name}.tar.gz`);
    run('tar',['-czf',archive,'-C',path.dirname(root),name]);
    notes.installers.push(path.basename(archive));
    const debRoot = path.join(output,'.deb-root');
    await fs.mkdir(path.join(debRoot,'DEBIAN'),{recursive:true});
    await fs.cp(root,path.join(debRoot,'usr/lib/creditia-gpui'),{recursive:true});
    await fs.mkdir(path.join(debRoot,'usr/bin'),{recursive:true});
    await fs.symlink('../lib/creditia-gpui/creditia-gpui',path.join(debRoot,'usr/bin/creditia-gpui'));
    await fs.mkdir(path.join(debRoot,'usr/share/applications'),{recursive:true});
    await fs.copyFile(path.join(root,'creditia.desktop'),path.join(debRoot,'usr/share/applications/creditia-gpui.desktop'));
    const bytes = info.files.reduce((n,item)=>n+item.size,0);
    await fs.writeFile(path.join(debRoot,'DEBIAN/control'),`Package: creditia-gpui\nVersion: ${info.version}\nArchitecture: amd64\nMaintainer: WroughtMind\nInstalled-Size: ${Math.ceil(bytes/1024)}\nDepends: libc6 (>= 2.39), libfontconfig1, libxkbcommon0, libxkbcommon-x11-0, libvulkan1, libasound2t64, libssl3t64, libzstd1, libwayland-client0, libx11-xcb1, xdg-desktop-portal, xdg-desktop-portal-gtk | xdg-desktop-portal-kde | xdg-desktop-portal-gnome | xdg-desktop-portal-xapp\nSection: devel\nPriority: optional\nDescription: Creditia native GPUI desktop\n Includes the artwork renderer, fonts and Node runtime.\n`);
    const deb = path.join(output,`${name}.deb`);
    run('dpkg-deb',['--root-owner-group','--build',debRoot,deb]);
    run('dpkg-deb',['--info',deb]);
    await fs.rm(debRoot,{recursive:true});
    notes.installers.push(path.basename(deb));
  }
  const checksums = [];
  for (const file of notes.installers) {
    const bytes = await fs.readFile(path.join(output,file));
    checksums.push(`${createHash('sha256').update(bytes).digest('hex')}  ${file}`);
  }
  await fs.writeFile(path.join(output,'SHA256SUMS'),checksums.join('\n')+'\n');
  await fs.writeFile(path.join(output,'candidate.json'),JSON.stringify(notes,null,2)+'\n');
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main(process.argv.slice(2));
}
