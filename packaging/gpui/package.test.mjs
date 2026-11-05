import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { seal, verify, layout, files, bridgeRequest } from './package.mjs';

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
