// 作品上的悬停交互：App 和导出的 HTML 用同一份代码。
// 画面是一张 SVG；交互层是盖在上面的一层透明区域（hits），悬停时显示提示、同一贡献方的区域一起亮起。

export interface HitLike { x: number; y: number; w: number; h: number; k?: string; t: string; d?: string[] }

export const INTERACT_CSS = `
.ct-art{position:relative;display:inline-block;line-height:0}
.ct-art>svg{display:block;width:100%;height:auto}
.ct-hits{position:absolute;inset:0}
.ct-hit{position:absolute;border-radius:10px;transition:box-shadow .18s ease,background-color .18s ease;cursor:default}
.ct-hit.on{box-shadow:0 0 0 1.5px rgba(255,255,255,.95),0 0 0 3px rgba(0,0,0,.35);background:rgba(255,255,255,.06)}
.ct-hit.kin{box-shadow:0 0 0 1px rgba(255,255,255,.55),0 0 0 2px rgba(0,0,0,.18)}
.ct-tip{position:fixed;z-index:50;pointer-events:none;min-width:180px;max-width:320px;padding:10px 12px 11px;border-radius:14px;
  font:12.5px/1.5 Inter,-apple-system,"PingFang SC","Noto Sans SC",sans-serif;color:#F5F5F4;
  background:rgba(18,18,18,.72);backdrop-filter:blur(18px) saturate(1.6);-webkit-backdrop-filter:blur(18px) saturate(1.6);
  box-shadow:0 10px 30px rgba(0,0,0,.28),inset 0 0 0 1px rgba(255,255,255,.14);opacity:0;transform:translateY(4px);transition:opacity .14s,transform .14s}
.ct-tip.on{opacity:1;transform:none}
.ct-tip b{display:block;font-weight:600;font-size:13px;margin-bottom:3px}
.ct-tip span{display:block;color:rgba(245,245,244,.72)}
`;

/** 提示框跟着指针，靠近窗口边缘时翻到另一侧 */
export function placeTip(tip: HTMLElement, x: number, y: number) {
  const tw = tip.offsetWidth, th = tip.offsetHeight;
  let tx = x + 16, ty = y + 18;
  if (tx + tw > innerWidth - 8) tx = x - tw - 16;
  if (ty + th > innerHeight - 8) ty = y - th - 18;
  tip.style.left = tx + 'px'; tip.style.top = ty + 'px';
}

/** 在 root（包着 SVG 的元素）上挂交互层。w、h 是作品的原始尺寸，区域按比例摆放 */
export function mountHits(root: HTMLElement, hits: HitLike[], w: number, h: number, positionTip = placeTip): () => void {
  const layer = document.createElement('div');
  layer.className = 'ct-hits';
  let tip = document.querySelector('.ct-tip') as HTMLElement | null;
  if (!tip) { tip = document.createElement('div'); tip.className = 'ct-tip'; document.body.appendChild(tip); }
  const els: HTMLElement[] = [];
  const esc = (s: string) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' } as any)[c]);
  for (const x of hits) {
    const e = document.createElement('div');
    e.className = 'ct-hit';
    e.style.left = (x.x / w) * 100 + '%';
    e.style.top = (x.y / h) * 100 + '%';
    e.style.width = (x.w / w) * 100 + '%';
    e.style.height = (x.h / h) * 100 + '%';
    // 进入时先定好位置再显示：热区可能是在静止的指针下面才挂上的，等不到下一次 mousemove
    e.onmouseenter = (ev) => {
      for (const o of els) o.classList.toggle('kin', !!x.k && (o as any)._k === x.k && o !== e);
      e.classList.add('on');
      tip!.innerHTML = `<b>${esc(x.t)}</b>${(x.d ?? []).map((l) => `<span>${esc(l)}</span>`).join('')}`;
      positionTip(tip!, ev.clientX, ev.clientY);
      tip!.classList.add('on');
    };
    e.onmousemove = (ev) => positionTip(tip!, ev.clientX, ev.clientY);
    e.onmouseleave = () => { e.classList.remove('on'); for (const o of els) o.classList.remove('kin'); tip!.classList.remove('on'); };
    (e as any)._k = x.k;
    els.push(e);
    layer.appendChild(e);
  }
  root.appendChild(layer);
  return () => { layer.remove(); tip?.classList.remove('on'); };
}
