#!/usr/bin/env node
// Archive and native installer generation is separate from staging/sealing.
import fs from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { verify } from './package.mjs';

const options = Object.fromEntries(process.argv.slice(2).reduce((pairs, arg, i, all) => i % 2 ? pairs : [...pairs, [arg.replace(/^--/, ''), all[i + 1]]], []));
if (!options.root || !options.output) throw new Error('Usage: installers.mjs --root package-directory --output installer-directory');
const root = path.resolve(options.root), output = path.resolve(options.output);
const info = await verify(root), name = path.basename(root);
await fs.mkdir(output, { recursive: true });
const run = (program, args, settings = {}) => execFileSync(program, args, { stdio: 'inherit', ...settings });
const notes = { target: info.target, candidate_sha: info.commit, installers: [], signing: info.platform === 'darwin' ? 'ad-hoc macOS signature; not notarized' : 'unsigned candidate' };

if (info.platform === 'darwin') {
  const archive = path.join(output, `${name}.zip`);
  run('ditto', ['-c','-k','--sequesterRsrc','--keepParent',root,archive]);
  notes.installers.push(path.basename(archive));
  const dmgSource = path.join(output, '.dmg-source');
  await fs.mkdir(dmgSource, { recursive: true });
  await fs.cp(path.join(root,'Creditia.app'), path.join(dmgSource,'Creditia.app'), { recursive: true });
  await fs.copyFile(path.join(root,'INSTALL.md'), path.join(dmgSource,'INSTALL.md'));
  await fs.copyFile(path.join(root,'package-info.json'), path.join(dmgSource,'package-info.json'));
  await fs.symlink('/Applications', path.join(dmgSource,'Applications'));
  const dmg = path.join(output, `${name}.dmg`);
  run('hdiutil', ['create','-volname','Creditia GPUI','-srcfolder',dmgSource,'-ov','-format','UDZO',dmg]);
  run('hdiutil', ['verify',dmg]);
  await fs.rm(dmgSource,{recursive:true});
  notes.installers.push(path.basename(dmg));
} else if (info.platform === 'win32') {
  const archive = path.join(output,`${name}.zip`);
  const tar = path.join(process.env.SystemRoot, 'System32', 'tar.exe');
  run(tar,['-a','-c','-f',archive,'-C',path.dirname(root),name]);
  notes.installers.push(path.basename(archive));
  const nsis = process.env.MAKENSIS ?? 'C:\\Program Files (x86)\\NSIS\\makensis.exe';
  await fs.access(nsis);
  const revision = `${info.version}-${(info.commit ?? 'local').slice(0,8)}`;
  const exe = path.join(output,`${name}-setup.exe`);
  const escape = value => value.replace(/\$/g,'$$').replace(/"/g,'$\\"');
  const script = `Unicode true\n!include "MUI2.nsh"\nName "Creditia GPUI ${revision}"\nOutFile "${escape(exe)}"\nInstallDir "$LOCALAPPDATA\\Programs\\Creditia GPUI\\${revision}"\nRequestExecutionLevel user\nSetCompressor /SOLID lzma\n!insertmacro MUI_PAGE_DIRECTORY\n!insertmacro MUI_PAGE_INSTFILES\n!insertmacro MUI_UNPAGE_CONFIRM\n!insertmacro MUI_UNPAGE_INSTFILES\n!insertmacro MUI_LANGUAGE "English"\nSection "Install"\nSetOutPath "$INSTDIR"\nFile /r "${escape(root)}\\*"\nCreateDirectory "$SMPROGRAMS\\Creditia GPUI"\nCreateShortCut "$SMPROGRAMS\\Creditia GPUI\\Creditia ${revision}.lnk" "$INSTDIR\\creditia-gpui.exe"\nWriteUninstaller "$INSTDIR\\uninstall.exe"\nWriteRegStr HKCU "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CreditiaGPUI-${revision}" "DisplayName" "Creditia GPUI ${revision}"\nWriteRegStr HKCU "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CreditiaGPUI-${revision}" "UninstallString" '"$INSTDIR\\uninstall.exe"'\nWriteRegStr HKCU "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CreditiaGPUI-${revision}" "DisplayVersion" "${info.version}"\nSectionEnd\nSection "Uninstall"\nDelete "$SMPROGRAMS\\Creditia GPUI\\Creditia ${revision}.lnk"\nRMDir "$SMPROGRAMS\\Creditia GPUI"\nDeleteRegKey HKCU "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\CreditiaGPUI-${revision}"\nRMDir /r "$INSTDIR"\nSectionEnd\n`;
  const source = path.join(output,'installer.nsi');
  await fs.writeFile(source,script);
  run(nsis,['/V2',source]);
  await fs.rm(source);
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
  await fs.writeFile(path.join(debRoot,'DEBIAN/control'),`Package: creditia-gpui\nVersion: ${info.version}\nArchitecture: amd64\nMaintainer: WroughtMind\nInstalled-Size: ${Math.ceil(bytes/1024)}\nDepends: libc6 (>= 2.39), libfontconfig1, libxkbcommon0, libxkbcommon-x11-0, libvulkan1, libasound2t64, libssl3t64, libzstd1, libwayland-client0, libwayland-cursor0, libx11-xcb1, libwebkit2gtk-4.1-0, xdg-desktop-portal, xdg-desktop-portal-gtk | xdg-desktop-portal-kde | xdg-desktop-portal-gnome | xdg-desktop-portal-xapp\nSection: devel\nPriority: optional\nDescription: Creditia native GPUI desktop\n Includes the artwork renderer, fonts and Node runtime.\n`);
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
