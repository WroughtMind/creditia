#!/usr/bin/env node
// Copy the license and notice files shipped with the exact resolved Rust crates.
import fs from 'node:fs/promises';
import path from 'node:path';
const args=Object.fromEntries(process.argv.slice(2).reduce((out,x,i,a)=>i%2?out:[...out,[x.replace(/^--/,''),a[i+1]]],[]));
if(!args.root||!args.metadata)throw new Error('Usage: licenses.mjs --root package --metadata rust-metadata.json');
const root=path.resolve(args.root),metadata=JSON.parse(await fs.readFile(args.metadata,'utf8'));
const info=JSON.parse(await fs.readFile(path.join(root,'package-info.json'),'utf8'));
const resources=info.platform==='darwin'?path.join(root,'Creditia.app/Contents/Resources'):path.join(root,'resources');
const output=path.join(resources,'licenses/Rust');await fs.mkdir(output,{recursive:true});
let previous=[];
try { previous=JSON.parse(await fs.readFile(path.join(output,'index.json'),'utf8')).crates; }
catch(error) {if(error.code!=='ENOENT')throw error;}
const entries=new Map(previous.map(entry=>[`${entry.name}@${entry.version}`,entry]));
for(const pkg of metadata.packages){
  const base=path.dirname(pkg.manifest_path),name=`${pkg.name}-${pkg.version}`;
  // Cargo versions include SemVer build metadata (ash 0.38.0+1.3.281,
  // wasi 0.11.1+wasi-snapshot-preview1). Validate each path component's
  // vocabulary independently rather than dropping that part of the identity.
  if(!/^[A-Za-z0-9][A-Za-z0-9_-]*$/.test(pkg.name)||!/^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?(?:\+[A-Za-z0-9.-]+)?$/.test(pkg.version))throw new Error(`Unexpected crate identifier: ${JSON.stringify([pkg.name,pkg.version])}`);
  const names=(await fs.readdir(base,{withFileTypes:true})).filter(x=>x.isFile()&&/^(licen[sc]e|copying|notice)([._-]|$)/i.test(x.name)).map(x=>x.name);
  if(pkg.license_file&&!names.includes(pkg.license_file)&&!path.isAbsolute(pkg.license_file)&&!pkg.license_file.split(/[\\/]/).includes('..'))names.push(pkg.license_file);
  const copied=[];
  for(const file of names){
    const bytes=await fs.readFile(path.join(base,file));if(bytes.length>2*1024*1024)throw new Error(`Unexpected license size: ${name}/${file}`);
    const target=path.join(output,name,file);await fs.mkdir(path.dirname(target),{recursive:true});await fs.writeFile(target,bytes);copied.push(file);
  }
  entries.set(`${pkg.name}@${pkg.version}`,{name:pkg.name,version:pkg.version,license:pkg.license,repository:pkg.repository,license_files:copied});
}
await fs.writeFile(path.join(output,'index.json'),JSON.stringify({schema:1,crates:[...entries.values()].sort((a,b)=>`${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`))},null,2)+'\n');
console.log(`Recorded ${entries.size} resolved Rust crate licenses/notices`);
