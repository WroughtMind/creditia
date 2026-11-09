import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {fileURLToPath} from 'node:url';

test('resolved crate licenses retain SemVer build metadata and reject path components',async()=>{
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'creditia-resolved-licenses-'));
  try {
    const crate=path.join(root,'crate'),bundle=path.join(root,'bundle'),metadata=path.join(root,'metadata.json');
    await fs.mkdir(crate);await fs.mkdir(bundle);
    await fs.writeFile(path.join(crate,'LICENSE-MIT'),'explicit fixture license\n');
    await fs.writeFile(path.join(bundle,'package-info.json'),JSON.stringify({platform:'linux'}));
    const pkg={name:'ash',version:'0.38.0+1.3.281',manifest_path:path.join(crate,'Cargo.toml'),license:'MIT'};
    const run=()=>promisify(execFile)(process.execPath,[fileURLToPath(new URL('./licenses.mjs',import.meta.url)),'--root',bundle,'--metadata',metadata]);
    await fs.writeFile(metadata,JSON.stringify({packages:[pkg]}));await run();
    const licenses=path.join(bundle,'resources/licenses/Rust');
    assert.equal(await fs.readFile(path.join(licenses,'ash-0.38.0+1.3.281/LICENSE-MIT'),'utf8'),'explicit fixture license\n');
    assert.equal(JSON.parse(await fs.readFile(path.join(licenses,'index.json'))).crates[0].version,pkg.version);
    for(const invalid of [{name:'../other'},{version:'1.0.0+../escape'}]){
      await fs.writeFile(metadata,JSON.stringify({packages:[{...pkg,...invalid}]}));
      await assert.rejects(run(),/Unexpected crate identifier/);
    }
  }finally{await fs.rm(root,{recursive:true,force:true});}
});
