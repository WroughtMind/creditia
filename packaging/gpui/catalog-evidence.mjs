#!/usr/bin/env node
// Contact sheets contain cropped, scaled native captures, never rerendered art.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { raster } from '../../packages/render/src/index.ts';
import { args, sha256 } from '../gpui-acceptance/common.mjs';

const options=args();
if(!options.captures||!options.output)throw new Error('Use --captures native-capture-directory --output evidence-directory');
const captures=path.resolve(options.captures),output=path.resolve(options.output);
const manifest=JSON.parse(await fs.readFile(path.join(captures,'capture-manifest.json'),'utf8'));
if(manifest.kind!=='native-catalog'||!Array.isArray(manifest.rows))throw new Error('Expected native catalog capture records');
await fs.mkdir(path.join(output,'contacts'),{recursive:true});
const escape=value=>String(value).replace(/[&<>"']/g,char=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&apos;'}[char]));
const width=340,height=260,columns=3,perSheet=12,contactFiles=[],captureFiles=[];
for(let start=0;start<manifest.rows.length;start+=perSheet){
  const rows=manifest.rows.slice(start,start+perSheet),blocks=[];
  for(const [index,row]of rows.entries()){
    const left=(index%columns)*width,top=Math.floor(index/columns)*height;
    blocks.push(`<rect x="${left+4}" y="${top+4}" width="${width-8}" height="${height-8}" rx="6" fill="#ffffff" stroke="#ccd0d4"/>`);
    const label=row.row.split('/');
    blocks.push(`<text x="${left+12}" y="${top+21}" font-family="Inter" font-size="12" fill="#222">${escape(label.slice(0,2).join('/'))}</text><text x="${left+12}" y="${top+39}" font-family="Inter" font-size="12" fill="#222">${escape(label.slice(2).join('/'))}</text>`);
    if(row.capture){
      if(path.basename(row.capture)!==row.capture)throw new Error('Unsafe native capture path');
      const bytes=await fs.readFile(path.join(captures,row.capture));
      if(!bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10])))throw new Error('Native screenshot is not PNG');
      const [windowWidth,windowHeight]=row.window_pixels;
      if(bytes.readUInt32BE(16)!==windowWidth||bytes.readUInt32BE(20)!==windowHeight)throw new Error('Native screenshot dimensions disagree with the capture record');
      const [x,y,w,h]=row.pixel_bounds;
      if(![x,y,w,h].every(Number.isFinite)||w<1||h<1||x<0||y<0||x+w>windowWidth||y+h>windowHeight)throw new Error('Invalid native artwork crop');
      row.screenshot_sha256=sha256(bytes);
      row.screenshot_bytes=bytes.length;
      row.screenshot={archive:'captures.tar.gz',path:row.capture,sha256:row.screenshot_sha256,bytes:bytes.length};
      captureFiles.push(row.capture);
      row.contact_cell={x:left+12,y:top+48,width:width-24,height:height-76};
      blocks.push(`<svg x="${left+12}" y="${top+48}" width="${width-24}" height="${height-76}" viewBox="${x} ${y} ${w} ${h}" preserveAspectRatio="xMidYMid meet"><image width="${windowWidth}" height="${windowHeight}" href="data:image/png;base64,${bytes.toString('base64')}"/></svg>`);
      delete row.capture;
    }else blocks.push(`<text x="${left+12}" y="${top+100}" font-family="Inter" font-size="14" fill="#a12222">${escape(row.status)}</text>`);
    blocks.push(`<text x="${left+12}" y="${top+height-12}" font-family="Inter" font-size="11" fill="${row.status==='passed'?'#126338':'#a12222'}">${escape(row.status)}</text>`);
  }
  const name=`contacts/sheet-${String(start/perSheet+1).padStart(3,'0')}.png`;
  const svg=`<svg xmlns="http://www.w3.org/2000/svg" width="${columns*width}" height="${Math.ceil(rows.length/columns)*height}"><rect width="100%" height="100%" fill="#eef0f2"/>${blocks.join('')}</svg>`;
  const bytes=raster.png(svg);
  await fs.writeFile(path.join(output,name),bytes);
  const artifact={path:name,sha256:sha256(bytes),bytes:bytes.length};
  for(const row of rows)row.artifacts=[artifact];
  contactFiles.push(name);
  console.log(`${name}: ${rows.length} native captures`);
}
// Preserve every full-resolution window alongside compact contacts. The sealed
// archive includes original row-to-file mapping, so screenshot hashes remain
// independently verifiable after the builder has been destroyed.
const archive=path.join(output,'captures.tar.gz');
execFileSync('tar',['-czf',archive,'-C',captures,'--','capture-manifest.json',...new Set(captureFiles)],{timeout:600_000,stdio:'pipe'});
const archiveHash=createHash('sha256');
for await(const chunk of createReadStream(archive))archiveHash.update(chunk);
const archiveRecord={path:'captures.tar.gz',sha256:archiveHash.digest('hex'),bytes:(await fs.stat(archive)).size};
manifest.artifacts=[archiveRecord];
manifest.full_resolution={archive:archiveRecord,captures:captureFiles.length,png_bytes:manifest.rows.reduce((sum,row)=>sum+(row.screenshot_bytes??0),0)};
console.log(`Retained ${captureFiles.length} full-resolution PNGs: ${manifest.full_resolution.png_bytes} bytes, archive ${archiveRecord.bytes} bytes`);
manifest.complete=manifest.complete===true&&manifest.rows.length===manifest.scope.selected_rows&&manifest.rows.every(row=>row.status==='passed'&&row.decoded===true&&row.screenshot_sha256);
manifest.review={contact_sheets:contactFiles.length,source:'Cropped/scaled pixels from real GPUI window captures; each full window PNG is retained in the hashed captures.tar.gz archive',preference:'Chinese, automatic grouping, visible contributor names; language/theme permutations are covered by the separate native-ui suite'};
manifest.cases=manifest.rows.map(row=>({...row,key:row.row}));
delete manifest.rows;
await fs.writeFile(path.join(output,'manifest.json'),JSON.stringify(manifest,null,2)+'\n');
const html=`<!doctype html><meta charset="utf-8"><title>Creditia native catalog</title><style>body{font:16px system-ui;background:#eef0f2;color:#17212b;margin:24px auto;max-width:1040px}img{display:block;width:100%;height:auto;margin:24px 0}p{line-height:1.5}a{color:#145fa0}</style><h1>Native desktop catalog</h1><p>${manifest.cases.length} rows · complete=${manifest.complete} · ${escape(manifest.provenance.candidate_sha??'local')}</p><p>Every tile below is a crop of a real GPUI focused artwork after production piece/layout input. Metadata and full-window PNG hashes are recorded in <a href="manifest.json">manifest.json</a>. Download <a href="captures.tar.gz">all full-resolution native captures</a> for individual inspection.</p>${contactFiles.map(file=>`<img loading="lazy" src="${file}" alt="Native artwork contact sheet">`).join('')}`;
await fs.writeFile(path.join(output,'index.html'),html);
if(!manifest.complete)process.exitCode=1;
