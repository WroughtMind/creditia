#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
const root=path.resolve(process.argv[2]??'evidence/native');
const file=path.join(root,'manifest.json');
const report=JSON.parse(await fs.readFile(file,'utf8'));
for(const item of report.cases){
  for(const artifact of item.artifacts??[]){
    if(path.isAbsolute(artifact.path)||artifact.path.split(/[\\/]/).includes('..'))throw new Error('Unsafe evidence path');
    const bytes=await fs.readFile(path.join(root,artifact.path));
    artifact.sha256=createHash('sha256').update(bytes).digest('hex');artifact.bytes=bytes.length;
  }
}
report.complete=report.cases.length>0&&report.cases.every(c=>c.status==='passed');
await fs.writeFile(file,JSON.stringify(report,null,2)+'\n');
console.log(`${report.cases.length} native cases, complete=${report.complete}`);
