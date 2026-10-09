// creditia App 前端：项目列表 + tier 预览 + 展示选项。预览复用 <creditia-card>（同一份数据模型）。
import '../../../packages/embed/src/creditia-card.ts';

const inv = (window as any).__TAURI__?.core?.invoke ?? (async (c: string, a: any) => {
  // 浏览器里直接跑（vite dev 无 Tauri）时给一份提示，方便调样式
  if (c === 'projects') return [];
  throw new Error('需要在 creditia App 里运行');
});

const THEMES = ['minimal-light', 'minimal-dark', 'film', 'terminal', 'glass', 'editorial', 'mono'];
const $ = (s: string) => document.querySelector(s)!;
let current: any = null;
let credits: any = null;

function renderList(ps: any[]) {
  const ul = $('#projects') as HTMLUListElement;
  ul.innerHTML = '';
  for (const p of ps) {
    const li = document.createElement('li');
    li.className = p.id === current?.id ? 'on' : '';
    li.innerHTML = `<b>${p.name}</b><span>${p.lastAnalyzed ? new Date(p.lastAnalyzed * 1000).toLocaleDateString('zh-CN') : '未分析'} · ${p.evidence} 条证据</span>`;
    li.onclick = () => open(p);
    ul.append(li);
  }
  $('#lib')!.textContent = ps.length ? `作品库：${ps.length} 个项目` : '作品库为空；在仓库里运行 creditia analyze';
}

async function open(p: any) {
  current = p;
  renderList(await inv('projects'));
  try {
    credits = await inv('credits', { projectId: p.id });
  } catch (e: any) {
    ($('#empty') as HTMLElement).hidden = false;
    $('#empty')!.textContent = String(e?.message ?? e);
    ($('#view') as HTMLElement).hidden = true;
    return;
  }
  ($('#empty') as HTMLElement).hidden = true;
  ($('#view') as HTMLElement).hidden = false;
  $('#name')!.textContent = p.name;
  $('#meta')!.textContent = `${credits.totals?.commits ?? 0} 次提交 · ${(credits.totals?.linesAdded ?? 0).toLocaleString()} 行 · 生成于 ${String(credits.generatedAt).slice(0, 10)}`;
  await showCard();
}

async function showCard() {
  const wrap = $('#card-wrap')!;
  const theme = ($('#theme') as HTMLSelectElement).value;
  const director = ($('#director') as HTMLInputElement).checked;
  const cfg = await inv('config_of', { projectId: current.id }).catch(() => ({}));
  const data = JSON.parse(JSON.stringify(credits));
  // 主导者开关只在预览时注入/摘掉名字，不改档案
  if (director && !data.directors?.length && cfg?.directors?.length) data.directors = cfg.directors;
  if (!director) data.directors = [];
  wrap.innerHTML = '';
  const el = document.createElement('creditia-card') as any;
  el.credits = data;
  el.setAttribute('theme', theme);
  el.setAttribute('logos', '/');
  if (director) el.setAttribute('director', '');
  wrap.append(el);
}

function wire() {
  const sel = $('#theme') as HTMLSelectElement;
  sel.innerHTML = THEMES.map((t) => `<option>${t}</option>`).join('');
  sel.value = localStorage.theme ?? 'minimal-light';
  sel.onchange = () => {
    localStorage.theme = sel.value;
    showCard();
  };
  ($('#director') as HTMLInputElement).onchange = showCard;
  ($('#copy-embed') as HTMLButtonElement).onclick = async () => {
    const s = await inv('embed_snippet', { projectId: current.id, form: 'tier', theme: sel.value, director: ($('#director') as HTMLInputElement).checked });
    await navigator.clipboard.writeText(s);
    const b = $('#copy-embed') as HTMLButtonElement;
    b.textContent = '已复制';
    setTimeout(() => (b.textContent = '复制嵌入代码'), 1500);
  };
  ($('#export-hint') as HTMLButtonElement).onclick = () => {
    alert(`在项目目录运行：\ncreditia export --form tier --theme ${sel.value} --format png --out tier.png\ncreditia export --list 看全部形式和风格`);
  };
}

wire();
inv('projects').then(renderList).catch((e: any) => ($('#lib')!.textContent = String(e?.message ?? e)));
