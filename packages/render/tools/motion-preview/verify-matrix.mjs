import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {Resvg} from '@resvg/resvg-js';
import {FONTS,familiesIn} from '../../src/fonts.ts';
import {createHash} from 'node:crypto';
import {art,frames,buildModel,PIECES,STYLES,raster,fontFiles} from '../../src/index.ts';
import {THEMES} from '../../src/classic/themes.ts';
import {bridgeStyle,BRIDGED,BRIDGED_LAYOUTS} from '../../src/classic/bridge.ts';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../../../..');
const dir=root+'/output/playwright/material-matrix';mkdirSync(dir,{recursive:true});
const allFiles=fontFiles();
const pngOf=svg=>{
 const names=new Set([...svg.matchAll(/font-family="([^"]+)"/g)].flatMap(m=>familiesIn(m[1].replace(/&quot;/g,'"').replace(/&apos;/g,"'")))); names.add('Noto Sans SC');
 const needed=FONTS.filter(f=>names.has(f.family));
 const files=allFiles.filter(p=>needed.some(f=>p.includes('/'+f.pkg+'/'+f.dir+'/')));
 return new Resvg(svg,{font:{fontFiles:files,loadSystemFonts:false,defaultFontFamily:'Noto Sans SC'},fitTo:{mode:'width',value:120},shapeRendering:2,textRendering:1,imageRendering:0}).render().asPng();
};
const m=buildModel(JSON.parse(readFileSync(root+'/packages/render/fixtures/catnap.json','utf8')));
const norm=s=>s.replace(/\bc[0-9a-z]+-/g,'scene-');
const modern=STYLES.filter(s=>!['minimal','noir','glass','cinema','terminal'].includes(s.id));
const bridge=THEMES.map(bridgeStyle);
const cases=modern.flatMap(st=>Object.entries(PIECES).flatMap(([id,p])=>(p.variants??['']).map(variant=>({st,id,variant}))));
cases.push(...bridge.flatMap(st=>[...BRIDGED.map(id=>({st,id,variant:''})),...Object.entries(BRIDGED_LAYOUTS).flatMap(([id,variants])=>variants.map(variant=>({st,id,variant})))]));
// 同一 SVG 的所需字体子集须与导出器全字体结果逐像素一致。
for(const st of modern){const a=art('tier',m,st,{t:.35});if(Buffer.compare(pngOf(a.svg),Buffer.from(raster.png(a.svg,120))))throw Error('font subset differs: '+st.id);}
const checks=[],failures=[];
for(const [i,{st,id,variant}]of cases.entries()){
 const key=[st.id,id,variant||'default'].join('/');
 writeFileSync(dir+'/current.txt',key);
 try{
  const aa=[0,.35,.7,1].map(t=>art(id,m,st,{t,variant:variant||undefined}));
  if(aa.some(a=>!a))throw Error('no artwork');
  if(new Set(aa.map(a=>`${a.w}/${a.h}`)).size!==1)throw Error('unstable dimensions');
  const hashes=aa.map(a=>createHash('sha256').update(norm(a.svg)).digest('hex'));
  let states = new Set(hashes).size;
  if (aa[0].motion?.kind === 'scroll') {
    const scroll = frames(id,m,st,{variant},4);
    states = new Set(scroll.map(a=>a.crop?.y)).size;
    if (new Set(scroll.map(a=>a.crop?.svg)).size !== 1) throw Error('scroll re-renders its reel');
  }
  if(states<2)throw Error('no motion');
  for(const a of aa){
   if(/NaN|Infinity/.test(a.svg))throw Error('non-finite SVG');
   if(a.hits.some(h=>!Number.isFinite(h.x+h.y+h.w+h.h)||h.w<0||h.h<0))throw Error('invalid hover');
   const png=pngOf(a.svg);if(png.length<100)throw Error('empty PNG');
  }
  checks.push({key,states,frames:aa.length});
 }catch(e){failures.push({key,error:e.stack??String(e)});console.log('FAIL',key,e.message);}
 if(i%30===0){console.log(`${i+1}/${cases.length}`);writeFileSync(dir+'/partial.json',JSON.stringify({checked:checks.length,failures,checks}));}
}
writeFileSync(dir+'/results.json',JSON.stringify({combinations:cases.length,frames:checks.length*4,failures,checks},null,2));
console.log(JSON.stringify({combinations:cases.length,frames:checks.length*4,failures}));
process.exitCode=failures.length?1:0;
