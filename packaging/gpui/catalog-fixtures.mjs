#!/usr/bin/env node
// Canonical input for the exhaustive Linux native view test, one style at a time.
import fs from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { renderRequest } from '../../packages/render/src/native-protocol.ts';
import { contexts, requestFor, preferences } from '../gpui-acceptance/fixtures.mjs';
import { args, caseKey, parseMatrix, sha256 } from '../gpui-acceptance/common.mjs';

const options=args(),root=path.resolve(import.meta.dirname,'../..');
const output=path.resolve(options.output??'evidence/native-catalog-inputs');
const source=path.resolve(options.fixture??path.join(root,'packages/render/fixtures/catnap.json'));
const bytes=await fs.readFile(source),raw=JSON.parse(bytes),context=contexts(raw),preference=preferences[0];
const all=parseMatrix(await fs.readFile(path.join(root,'docs/gpui-migration/matrix.tsv'),'utf8')).filter(row=>row.desktop==='true');
const selected=options.styles?new Set(String(options.styles).split(',')):null;
const rows=selected?all.filter(row=>selected.has(row.style)):all;
if(!rows.length)throw new Error('Native catalog selection is empty');
const response=await renderRequest({version:1,id:'catalog',op:'catalog'});
if(!response.ok)throw new Error(JSON.stringify(response));
const catalog=response.result;
await fs.mkdir(output,{recursive:true});
const manifest={schema_version:1,kind:'native-catalog-inputs',candidate_sha:process.env.GITHUB_SHA??null,fixture:{path:path.relative(root,source),sha256:sha256(bytes),kind:'synthetic_render_fixture'},preference,scope:{total_desktop_rows:all.length,selected_rows:rows.length,full:rows.length===all.length},batches:[]};
if(!options.styles){
  // Resvg/Satori font allocations are process-local. A fresh process per style
  // bounds memory without reducing artwork coverage or changing any request.
  for(const id of [...new Set(rows.map(row=>row.style))]){
    const relative=path.join('styles',id),child=path.join(output,relative);
    execFileSync(process.execPath,[path.join(import.meta.dirname,'catalog-fixtures.mjs'),'--styles',id,'--fixture',source,'--output',child],{stdio:'inherit',timeout:600_000});
    const batch=JSON.parse(await fs.readFile(path.join(child,'catalog-inputs.json'),'utf8'));
    for(const entry of batch.batches)manifest.batches.push({...entry,path:path.join(relative,entry.path).split(path.sep).join('/')});
  }
  await fs.writeFile(path.join(output,'catalog-inputs.json'),JSON.stringify(manifest,null,2)+'\n');
  process.exit(0);
}
for(const id of [...new Set(rows.map(row=>row.style))]){
  const style=catalog.styles.find(style=>style.id===id);
  if(!style)throw new Error(`Desktop style missing from production catalog: ${id}`);
  const artworks=[],records=[];
  for(const row of rows.filter(row=>row.style===id)){
    const request={...requestFor(row,raw,preference),flatten:true};
    const result=await renderRequest(request);
    if(!result.ok)throw new Error(`${caseKey(row)}: ${JSON.stringify(result.error)}`);
    const art=result.result.art??null;
    const pieceIndex=style.pieces.findIndex(piece=>piece.id===row.piece),piece=style.pieces[pieceIndex];
    if(!piece)throw new Error(`Piece missing from native selector: ${caseKey(row)}`);
    const variant=request.variant??piece.defaultVariant??piece.variants?.[0]?.id??null;
    const variantIndex=variant===null?null:piece.variants.findIndex(item=>item.id===variant);
    if(variantIndex===-1)throw new Error(`Layout missing from native selector: ${caseKey(row)}`);
    artworks.push({style:id,piece:row.piece,variant,art});
    records.push({row:caseKey(row),style:id,piece:row.piece,variant,piece_index:pieceIndex,variant_index:variantIndex,expected:art?{width:art.w,height:art.h,hit_count:art.hits.length}:null});
  }
  const fixture={raw,catalog,artworks,language:preference.lang,appearance:'light',reduced_motion:true,projects:context.projects,person:context.person,blame:context.blame};
  const data=Buffer.from(JSON.stringify(fixture)),name=`${id}.json`;
  await fs.writeFile(path.join(output,name),data);
  manifest.batches.push({style:id,path:name,sha256:sha256(data),bytes:data.length,rows:records});
  console.log(`${id}: ${records.length} actual native cases, ${records.filter(row=>!row.expected).length} empty outputs`);
}
await fs.writeFile(path.join(output,'catalog-inputs.json'),JSON.stringify(manifest,null,2)+'\n');
