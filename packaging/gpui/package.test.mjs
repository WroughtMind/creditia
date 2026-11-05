import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { stage, seal, verify, layout, files, bridgeRequest } from './package.mjs';

test('package manifest detects changed, removed and injected runtime files', async () => {
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'creditia-package-test-'));
  try {
    await fs.mkdir(path.join(root,'resources'),{recursive:true});
    await fs.writeFile(path.join(root,'resources/font.ttf'),'font-bytes');
    await fs.writeFile(path.join(root,'package-info.json'),JSON.stringify({schema:1,target:'test',fontFiles:['resources/font.ttf']}));
    await seal(root); await verify(root);
    await fs.writeFile(path.join(root,'resources/font.ttf'),'changed');
    await assert.rejects(verify(root),/checksum mismatch/);
    await fs.writeFile(path.join(root,'resources/font.ttf'),'font-bytes');
    await fs.writeFile(path.join(root,'resources/injected.mjs'),'');
    await assert.rejects(verify(root),/Unexpected package file/);
    await fs.rm(path.join(root,'resources/injected.mjs'));
    await fs.rm(path.join(root,'resources/font.ttf'));
    await assert.rejects(verify(root),/ENOENT/);
  } finally { await fs.rm(root,{recursive:true,force:true}); }
});

test('platform layouts match native resource discovery without checkout paths', () => {
  assert.equal(layout('/candidate','darwin').bridge,path.join('/candidate','Creditia.app/Contents/Resources/render/tools/bridge.mjs'));
  assert.equal(layout('/candidate','win32').node,path.join('/candidate','resources/node.exe'));
  assert.equal(layout('/candidate','linux').core,path.join('/candidate','creditia-core'));
});

test('packaged child transport validates response ID and process failure', async () => {
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'creditia-child-test-'));
  const bridge=path.join(root,'bridge.mjs');
  try {
    await fs.writeFile(bridge,`let s=''; for await(const c of process.stdin)s+=c;const r=JSON.parse(s);console.log(JSON.stringify({version:1,id:r.id,ok:true,result:{styles:[]}}));`);
    assert.deepEqual(await bridgeRequest(process.execPath,bridge,{id:'ok'},root),{styles:[]});
    await fs.writeFile(bridge,`console.log(JSON.stringify({version:1,id:'wrong',ok:true,result:{}}));`);
    await assert.rejects(bridgeRequest(process.execPath,bridge,{id:'ok'},root),/wrong/);
    await fs.writeFile(bridge,`process.stderr.write('renderer missing');process.exit(3);`);
    await assert.rejects(bridgeRequest(process.execPath,bridge,{id:'ok'},root),/renderer missing/);
  } finally {await fs.rm(root,{recursive:true,force:true});}
});

test('package manifest rejects traversal and symbolic links', async () => {
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'creditia-path-test-'));
  try {
    await fs.writeFile(path.join(root,'package-info.json'),JSON.stringify({schema:1,fontFiles:[],files:[{path:'../outside',size:1,sha256:'x'}]}));
    await assert.rejects(verify(root),/Unsafe manifest path/);
    if(process.platform!=='win32'){
      await fs.symlink('/tmp',path.join(root,'outside'));
      await assert.rejects(files(root),/symlinks/);
    }
  } finally {await fs.rm(root,{recursive:true,force:true});}
});

test('staging preserves schema and required fonts while pruning previews and unused weights',async()=>{
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'creditia-stage-test-'));
  const write=async(relative,content)=>{const file=path.join(root,relative);await fs.mkdir(path.dirname(file),{recursive:true});await fs.writeFile(file,content);return file;};
  try{
    await write('source/apps/gpui/Cargo.toml','[package]\nversion = "0.1.0"\n');
    await write('source/packages/render/src/fonts.ts',`export const FONTS=[{pkg:'sample',dir:'400Regular'}];`);
    await write('source/packages/render/src/classic/fonts.ts',`export const CLASSIC_FONTS=[];`);
    await write('source/packages/render/tools/bridge.mjs','');
    await write('source/packages/render/package.json','{"type":"module"}');
    await write('source/packages/render/package-lock.json','{}');
    await write('source/packages/render/node_modules/@expo-google-fonts/sample/package.json','{}');
    await write('source/packages/render/node_modules/@expo-google-fonts/sample/OFL.txt','font license');
    await write('source/packages/render/node_modules/@expo-google-fonts/sample/font-family.png','unused');
    await write('source/packages/render/node_modules/@expo-google-fonts/sample/400Regular/Font.ttf','required face');
    await write('source/packages/render/node_modules/@expo-google-fonts/sample/400Regular/Font.png','preview');
    await write('source/packages/render/node_modules/@expo-google-fonts/sample/700Bold/Font.ttf','unused face');
    await write('source/packages/schema/agents.json','{"test":true}');
    await write('source/packaging/gpui/INSTALL.md','install');
    const app=await write('binaries/app','app'),core=await write('binaries/core','core'),license=await write('binaries/LICENSE','node license');
    const result=await stage({target:'test-platform',output:path.join(root,'output'),appBin:app,coreBin:core,nodeLicense:license,source:path.join(root,'source'),platform:'linux',migrationBin:app});
    const fonts=path.join(result.resources,'render/node_modules/@expo-google-fonts/sample');
    assert.equal(await fs.readFile(path.join(fonts,'400Regular/Font.ttf'),'utf8'),'required face');
    assert.equal(await fs.readFile(path.join(fonts,'OFL.txt'),'utf8'),'font license');
    await assert.rejects(fs.access(path.join(fonts,'700Bold')),/ENOENT/);
    await assert.rejects(fs.access(path.join(fonts,'font-family.png')),/ENOENT/);
    await assert.rejects(fs.access(path.join(fonts,'400Regular/Font.png')),/ENOENT/);
    assert.equal(await fs.readFile(path.join(result.resources,'schema/agents.json'),'utf8'),'{"test":true}');
    assert.equal(await fs.readFile(path.join(result.resources,'legacy/creditia-migrate'),'utf8'),'app');
    await assert.rejects(stage({target:'test-platform',output:path.join(root,'output'),appBin:app,coreBin:core,nodeLicense:license,source:path.join(root,'source'),platform:'linux'}),/nonempty/);
  }finally{await fs.rm(root,{recursive:true,force:true});}
});
