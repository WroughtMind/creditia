#!/usr/bin/env node
// Produce canonical artwork inputs for the actual GPUI view. These are source
// fixtures, not screenshots and not a substitute for native pixel verification.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { renderRequest } from '../../packages/render/src/native-protocol.ts';

const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const args=Object.fromEntries(process.argv.slice(2).reduce((out,x,i,a)=>i%2?out:[...out,[x.replace(/^--/,''),a[i+1]]],[]));
const output=path.resolve(args.output??'evidence/fixtures');
const fixture=path.resolve(args.fixture??path.join(repo,'packages/render/fixtures/catnap.json'));
const input=await fs.readFile(fixture),raw=JSON.parse(input),hash=x=>createHash('sha256').update(x).digest('hex');
await fs.mkdir(output,{recursive:true});
const response=await renderRequest({version:1,id:'catalog',op:'catalog'});
if(!response.ok)throw new Error(JSON.stringify(response));
const catalog=response.result;
const selected=args.profile==='smoke'?new Set(['minimal-light','ink','cinema','cyber']):new Set(catalog.styles.map(s=>s.id));
const manifest={schema:1,candidate_sha:process.env.GITHUB_SHA??null,fixture:{path:path.relative(repo,fixture),sha256:hash(input),kind:args['fixture-kind']??'synthetic_render_fixture'},profile:args.profile??'full',files:[]};
for(const style of catalog.styles.filter(s=>selected.has(s.id))){
  for(const language of ['zh','en']){
    const artworks=[];
    const board=style.board??style.pieces.filter(p=>!p.requires?.length).map(p=>({id:p.id}));
    const requests=new Map();
    function add(entry,variant){const key=`${entry.id}#${variant??''}`;if(!requests.has(key))requests.set(key,{piece:entry.id,variant});}
    for(const entry of board){const piece=style.pieces.find(p=>p.id===entry.id);if(!piece||piece.requires?.length)continue;add(entry,entry.variant??piece.defaultVariant??piece.variants?.[0]?.id);}
    // Native captures show the initial board. The separate sharded matrix owns
    // every layout; only preload the one alternative exercised by native input.
    if(style.id==='minimal-light'&&language==='en'){
      const entry=board.find(entry=>entry.id==='tier'),piece=style.pieces.find(piece=>piece.id==='tier');
      const current=entry?.variant??piece?.defaultVariant??piece?.variants?.[0]?.id;
      const alternative=piece?.variants?.find(variant=>variant.id!==current);
      if(entry&&alternative)add(entry,alternative.id);
    }
    for(const item of requests.values()){
      const request={version:1,id:`${style.id}/${language}/${item.piece}/${item.variant??''}`,op:'render',raw,style:style.id,piece:item.piece,lang:language,group:'auto',named:true,flatten:true};
      if(item.variant)request.variant=item.variant;
      const rendered=await renderRequest(request);
      if(!rendered.ok)throw new Error(`${request.id}: ${JSON.stringify(rendered.error)}`);
      artworks.push({style:style.id,...item,art:rendered.result.art??null});
    }
    if(!artworks.some(entry=>entry.art))throw new Error(`No artworks generated for ${style.id}/${language}`);
    const bytes=Buffer.from(JSON.stringify({raw,catalog,artworks,language,appearance:'light',reduced_motion:true}));
    const name=`${style.id}-${language}.json`;
    await fs.writeFile(path.join(output,name),bytes);
    manifest.files.push({path:name,sha256:hash(bytes),bytes:bytes.length,style:style.id,language,artworks:artworks.length});
    console.log(`${name}: ${artworks.length} artworks, ${bytes.length} bytes`);
  }
}
await fs.writeFile(path.join(output,'fixture-manifest.json'),JSON.stringify(manifest,null,2)+'\n');
