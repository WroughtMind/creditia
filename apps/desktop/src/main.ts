import { THEMES, themeById } from '../../../packages/render/src/themes.ts';
import { buildModel, fmt, type Model } from '../../../packages/render/src/data.ts';
import { FORMS, type Ctx } from '../../../packages/render/src/registry.ts';
import { encodeExport, EXPORTS, motionSpec, type ExportFormat } from '../../../packages/render/src/export.ts';
import { renderSvg, renderSized, toRaster } from './preview.ts';
import { api, saveExport, launchContext, type Project } from './api.ts';
import { $, esc, icon, toast } from './dom.ts';
import { initPanels, showPanel, closePanel, type PanelController } from './panels.ts';
import { VARIANT_ZH, THEME_ZH } from './labels.ts';

type Preference = { variant?: string; size?: string };
const ORDER = ['tier', 'receipt', 'roll', 'share', 'byline', 'donut', 'ribbon', 'mosaic', 'waffle', 'area', 'lanes', 'clock', 'versions', 'settle', 'app', 'fetch', 'profile'];
const state = {
  projects: [] as Project[], library: [] as any[], current: null as Project | null, raw: null as any, config: {} as any, model: null as Model | null,
  theme: themeById(localStorage.getItem('creditiaStudioTheme') ?? '')?.id ?? 'minimal-light', selected: '', epoch: 0,
  preferences: {} as Record<string, Preference>, svgs: new Map<string, string>(), unavailable: [] as string[], exporting: false,
};
const theme = () => themeById(state.theme)!;
const preferenceKey = () => `creditiaStudio:${state.current?.id}`;
const context = (id: string): Ctx => ({ lang: 'zh', variant: state.preferences[id]?.variant, ...(id === 'profile' ? { projects: state.library.map(raw => raw.project?.id === state.raw.project?.id ? state.model! : buildModel(raw)), person: state.model!.signature } : {}) });
const errorMessage = (e: unknown) => e instanceof Error ? e.message : String(e);
const remember = () => localStorage.setItem(preferenceKey(), JSON.stringify(state.preferences));
function rebuildModel() { state.model = buildModel({ ...state.raw, people: (state.config.people ?? state.raw.people ?? []).map((p: any) => ({ ...p, name: state.config.privacy?.aliases?.[p.name] ?? p.name })) }, { people: !!state.config.display?.people, lang: 'zh' }); }
const panels = initPanels({ state, openProject, paintWorkspace, rebuildModel, empty } as PanelController);

async function openProject(project: Project) {
  if (!project) throw new Error('分析结果没有对应的项目记录');
  const epoch = ++state.epoch; state.current = project; state.selected = ''; state.svgs.clear(); state.model = null;
  $('#project-name').textContent = project.name; $('#workspace').innerHTML = '<div class="loading-state">正在读取项目记录…</div>'; updateDock();
  try {
    const [raw, config, library] = await Promise.all([api('credits', { projectId: project.id }), api('config_of', { projectId: project.id }), api('library_credits')]);
    if (epoch !== state.epoch) return;
    state.raw = raw; state.config = config; state.library = library.projects;
    try { state.preferences = JSON.parse(localStorage.getItem(preferenceKey()) ?? '{}'); } catch { state.preferences = {}; }
    rebuildModel(); paintStyles(); localStorage.setItem('creditiaStudioProject', project.id); await paintWorkspace();
  } catch (e) { if (epoch === state.epoch) empty(errorMessage(e), project.path); }
}
function empty(message = '把项目的制作过程，变成一套作品。', path = '') {
  $('#workspace').innerHTML = `<section class="welcome"><img src="/creditia/icon.png" width="76" height="76" alt="creditia"><h1>每一份投入，都留下名字。</h1><p>${esc(message)}</p><button class="primary" id="add-first">打开一个项目 ${icon('plus')}</button><span>读取本机记录 · 无需模型 · 无需上传</span></section>`;
  $('#add-first').onclick = () => panels.addProject(path);
}
async function paintWorkspace() {
  if (!state.model) return;
  const epoch = ++state.epoch, model = state.model, selectedTheme = theme();
  state.svgs.clear(); state.unavailable = []; updateDock();
  $('#workspace').innerHTML = `<div class="workspace-heading"><div><span class="eyebrow">制作档案</span><h1>${esc(model.project)}</h1></div><p>${fmt(model.commits)} 次提交<span>·</span>${model.activeDays} 个活跃日<span>·</span>${esc(model.range.from)} — ${esc(model.range.to)}</p></div><div class="work-board">${ORDER.map((id, i) => `<article class="artwork artwork-${id}" data-artifact="${id}" tabindex="0" aria-label="${esc(FORMS[id].name)}" aria-busy="true"><div class="art-caption"><span>${String(i + 1).padStart(2, '0')} / ${esc(FORMS[id].name)}</span><div class="art-actions"><button data-action="adjust" aria-label="调整${esc(FORMS[id].name)}" title="调整">${icon('adjust', 15)}</button><button data-action="expand" aria-label="放大${esc(FORMS[id].name)}" title="放大">${icon('expand', 15)}</button></div></div><div class="art-stage"><span class="rendering">正在排版</span></div></article>`).join('')}</div><button class="missing-info" id="missing-info" hidden></button>`;
  const board = $('.work-board');
  board.addEventListener('click', e => {
    const target = e.target as HTMLElement, item = target.closest<HTMLElement>('[data-artifact]'); if (!item) return;
    select(item.dataset.artifact!);
    const action = target.closest<HTMLElement>('[data-action]')?.dataset.action;
    if (action === 'adjust') adjust(state.selected); if (action === 'expand') enlarge(state.selected);
  });
  board.addEventListener('keydown', e => { if (e.key === 'Enter' || e.key === ' ') { const item = (e.target as HTMLElement).closest<HTMLElement>('[data-artifact]'); if (item && e.target === item) { e.preventDefault(); select(item.dataset.artifact!); } } });
  board.addEventListener('dragstart', e => {
    const item = (e.target as HTMLElement).closest<HTMLElement>('[data-artifact]');
    if (!item || !state.svgs.has(item.dataset.artifact!)) { e.preventDefault(); return; }
    select(item.dataset.artifact!); e.dataTransfer!.setData('application/x-creditia-form', state.selected); e.dataTransfer!.effectAllowed = 'copy';
    document.body.classList.add('dragging'); item.classList.add('drag-source');
  });
  board.addEventListener('dragend', endDrag);
  for (const id of ORDER) {
    if (epoch !== state.epoch) return;
    const item = board.querySelector<HTMLElement>(`[data-artifact="${id}"]`)!;
    try {
      const svg = await renderArtifact(id, model, selectedTheme, context(id), state.preferences[id]?.size);
      if (epoch !== state.epoch) return;
      if (!svg) { item.hidden = true; state.unavailable.push(id); continue; }
      state.svgs.set(id, svg); showSvg(item, svg, id); item.draggable = true; item.setAttribute('aria-busy', 'false');
    } catch (e) {
      if (epoch !== state.epoch) return;
      item.querySelector('.art-stage')!.innerHTML = `<div class="render-error">这张作品未能排版<br>${esc(errorMessage(e))}<button data-action="adjust">调整版式</button></div>`; item.setAttribute('aria-busy', 'false');
    }
    if (id === state.selected) updateDock();
    await new Promise(resolve => requestAnimationFrame(resolve));
  }
  if (state.unavailable.length) { const missing = $('#missing-info'); missing.hidden = false; missing.textContent = `${state.unavailable.length} 种作品还缺少对应记录`; missing.onclick = () => showPanel('等待记录', `<p>有对应记录后，这些作品会直接出现在这里。</p><ul>${state.unavailable.map(id => `<li><b>${esc(FORMS[id].name)}</b> · ${esc(FORMS[id].q)}</li>`).join('')}</ul>`); }
}
async function renderArtifact(id: string, model: Model, selectedTheme: ReturnType<typeof theme>, ctx: Ctx, size?: string) {
  if (size && size !== 'native') { const [w, h] = size.split('x').map(Number); if (!(Number.isFinite(w) && Number.isFinite(h) && w > 0 && h > 0)) throw new Error('请输入有效的宽和高'); return renderSized(id, model, selectedTheme, w, h, ctx); }
  return renderSvg(id, model, selectedTheme, ctx);
}
function showSvg(item: HTMLElement, svg: string, id: string) {
  const stage = item.querySelector<HTMLElement>('.art-stage')!, spec = motionSpec(svg, id); stage.style.aspectRatio = `${spec.width}/${spec.viewport}`;
  stage.innerHTML = `<div class="art-motion" style="--travel:-${spec.travel / spec.height * 100}%">${svg}</div>`;
}
function select(id: string) { state.selected = id; document.querySelectorAll<HTMLElement>('[data-artifact]').forEach(el => el.classList.toggle('selected', el.dataset.artifact === id)); updateDock(); }
function updateDock() {
  const ready = !!state.svgs.get(state.selected); $('#selection-name').textContent = state.selected ? `${FORMS[state.selected].name} · ${THEME_ZH[state.theme] ?? theme().name}` : '选择作品，或拖到这里导出';
  document.querySelectorAll<HTMLButtonElement>('[data-format]').forEach(b => b.disabled = !ready || state.exporting);
}
function endDrag() { document.body.classList.remove('dragging'); document.querySelectorAll('.drag-source,.drop-active').forEach(el => el.classList.remove('drag-source', 'drop-active')); }
function paintStyles() {
  $('#style-name').textContent = THEME_ZH[state.theme] ?? theme().name;
  $('#styles').innerHTML = THEMES.map(t => `<button class="style-dot ${t.id === state.theme ? 'active' : ''}" data-theme="${t.id}" aria-label="${esc(THEME_ZH[t.id] ?? t.name)}" aria-pressed="${t.id === state.theme}" style="--swatch:${`linear-gradient(130deg,${t.bg} 0% 45%,${t.accent} 47% 52%,${t.surface} 54%)`};--dot-ink:${t.ink}"><span class="dot-art" style="font-family:${esc(t.display)}">${t.family === 'terminal' || t.family === 'crt' ? '>_' : 'Aa'}</span><span class="dot-label">${esc(THEME_ZH[t.id] ?? t.name)}</span></button>`).join('');
  const model = state.model; if (model) void paintStylePreviews(model);
  $('#styles').onclick = e => { const button = (e.target as HTMLElement).closest<HTMLElement>('[data-theme]'); if (!button || button.dataset.theme === state.theme) return; state.theme = button.dataset.theme!; localStorage.setItem('creditiaStudioTheme', state.theme); closePanel(); paintStyles(); paintWorkspace(); };
}
let styleGeneration = 0;
async function paintStylePreviews(model: Model) {
  const generation = ++styleGeneration;
  for (const t of THEMES) {
    if (generation !== styleGeneration) return;
    try {
      const svg = await renderSvg('tier', model, t, { lang: 'zh' });
      if (generation !== styleGeneration) return;
      const dot = document.querySelector(`[data-theme="${t.id}"] .dot-art`);
      if (svg && dot) dot.innerHTML = svg;
    } catch { /* 圆点保留该主题的真实底色，作品区展示排版错误。 */ }
  }
}
function enlarge(id: string) { const svg = state.svgs.get(id); if (!svg) return; const panel = showPanel(FORMS[id].name, `<div class="large-preview">${svg}</div>`, 'preview-panel'); panel.querySelector('.large-preview')!.setAttribute('style', `background:${theme().bg}`); }
function adjust(id: string) {
  const form = FORMS[id], p = state.preferences[id] ?? {}, projectId = state.current?.id, epoch = state.epoch;
  const variants = form.variants ?? [], sizes = [['native','作品原始比例'],['1200x630','横版 · 链接预览'],['1080x1350','竖版 · 社交帖子'],['1080x1920','手机全屏'],['1080x1080','正方形'],['custom','自定尺寸']];
  const selectedSize = sizes.some(([v]) => v === (p.size ?? 'native')) ? (p.size ?? 'native') : 'custom';
  const panel = showPanel(form.name, `${variants.length ? `<label>版式<select id="variant"><option value="">随风格</option>${variants.map(v => `<option value="${v}" ${v === p.variant ? 'selected' : ''}>${esc(VARIANT_ZH[id]?.[v] ?? v)}</option>`).join('')}</select></label>` : ''}<label>画幅<select id="size">${sizes.map(([v,n]) => `<option value="${v}" ${v === selectedSize ? 'selected' : ''}>${n}</option>`).join('')}</select></label><div class="custom-size" id="custom-size" hidden><label>宽<input id="width" type="number" value="${p.size?.split('x')[0] ?? 1200}"></label><span>×</span><label>高<input id="height" type="number" value="${p.size?.split('x')[1] ?? 800}"></label></div><p class="field-error" id="adjust-error" hidden></p><button class="primary" id="apply">应用到这张作品</button>`);
  const size = panel.querySelector<HTMLSelectElement>('#size')!; const toggle = () => $('#custom-size').hidden = size.value !== 'custom'; size.onchange = toggle; toggle();
  $('#apply').onclick = async () => {
    const button = $('#apply') as HTMLButtonElement; button.disabled = true;
    try {
      const next = { variant: panel.querySelector<HTMLSelectElement>('#variant')?.value || undefined, size: size.value === 'custom' ? `${$<HTMLInputElement>('#width').value}x${$<HTMLInputElement>('#height').value}` : size.value };
      const svg = await renderArtifact(id, state.model!, theme(), { ...context(id), variant: next.variant }, next.size);
      if (!svg) throw new Error('还没有这种作品需要的记录');
      if (state.current?.id !== projectId || state.epoch !== epoch || !button.isConnected || !panel.open) return;
      state.preferences[id] = next; remember(); state.svgs.set(id, svg); const item = document.querySelector<HTMLElement>(`[data-artifact="${id}"]`); if (item) showSvg(item, svg, id); closePanel(); updateDock();
    } catch (e) { if (button.isConnected) { $('#adjust-error').textContent = errorMessage(e); $('#adjust-error').hidden = false; } } finally { button.disabled = false; }
  };
}
async function exportSelected(format: ExportFormat) {
  const svg = state.svgs.get(state.selected); if (!svg || state.exporting || !state.model) return;
  const snapshot = { svg, form: state.selected, title: `${state.model.project} · ${FORMS[state.selected].name}`, theme: theme(), project: state.model.project };
  state.exporting = true; updateDock(); toast(`正在制作 ${format.toUpperCase()}…`, true);
  try {
    const bytes = await encodeExport(snapshot.svg, format, toRaster, { title: snapshot.title, form: snapshot.form, background: snapshot.theme.bg, progress: value => toast(`正在制作动图 ${Math.round(value * 100)}%`, true) });
    const file = `${snapshot.project}-${snapshot.form}-${snapshot.theme.id}.${format}`.replace(/[\\/:*?"<>|]/g, '-');
    toast(await saveExport(file, bytes, EXPORTS.find(f => f.id === format)!.mime));
  } catch (e) { toast(`导出失败：${errorMessage(e)}`, true); } finally { state.exporting = false; updateDock(); }
}
async function boot() {
  $('#project-chevron').innerHTML = icon('chevron',14); $('#people-button').innerHTML = icon('people'); $('#info-button').innerHTML = icon('info'); $('#refresh-button').innerHTML = icon('refresh');
  $('#project-button').onclick = panels.projectMenu; $('#people-button').onclick = panels.peoplePanel; $('#info-button').onclick = panels.archivePanel; $('#refresh-button').onclick = panels.refresh;
  $('.wordmark').onclick = e => { e.preventDefault(); $('#workspace').scrollTo({ top: 0, behavior: 'smooth' }); };
  $('#exports').innerHTML = EXPORTS.map(f => `<button data-format="${f.id}" title="${f.description}" disabled>${icon('export',16)}<span>${f.label}</span></button>`).join('');
  $('#exports').onclick = e => { const button = (e.target as HTMLElement).closest<HTMLElement>('[data-format]'); if (button) exportSelected(button.dataset.format as ExportFormat); };
  $('#exports').addEventListener('dragover', e => { if (!e.dataTransfer?.types.includes('application/x-creditia-form')) return; e.preventDefault(); const button = (e.target as HTMLElement).closest<HTMLElement>('[data-format]'); document.querySelectorAll('.drop-active').forEach(x => x.classList.remove('drop-active')); button?.classList.add('drop-active'); e.dataTransfer.dropEffect = 'copy'; });
  $('#exports').addEventListener('drop', e => { e.preventDefault(); const form = e.dataTransfer?.getData('application/x-creditia-form'); const button = (e.target as HTMLElement).closest<HTMLElement>('[data-format]'); endDrag(); if (form && state.svgs.has(form) && button) { select(form); exportSelected(button.dataset.format as ExportFormat); } });
  document.addEventListener('keydown', e => { if (e.key === 'Escape') endDrag(); }); window.addEventListener('blur', endDrag);
  for (const dialog of document.querySelectorAll('dialog')) dialog.addEventListener('click', e => { if (e.target === dialog) { const rect = dialog.getBoundingClientRect(); if (e.clientX < rect.left || e.clientX > rect.right || e.clientY < rect.top || e.clientY > rect.bottom) dialog.close(); } });
  paintStyles();
  try {
    const launch = await launchContext();
    if (launch.theme && themeById(launch.theme)) { state.theme = launch.theme; paintStyles(); }
    state.projects = await api('projects');
    if (launch.repo) {
      let project = state.projects.find(p => p.path === launch.repo || p.paths?.includes(launch.repo!));
      if (!project) { const result = await api('analyze_project', { path: launch.repo }); state.projects = await api('projects'); project = state.projects.find(p => p.id === result.id); }
      if (!project) throw new Error('项目尚未写入作品库');
      await openProject(project);
    } else {
      const initial = state.projects.find(p => p.id === localStorage.getItem('creditiaStudioProject')) ?? state.projects[0];
      if (initial) await openProject(initial); else empty();
    }
    if (launch.form && state.svgs.has(launch.form)) { select(launch.form); document.querySelector(`[data-artifact="${launch.form}"]`)?.scrollIntoView({ block: 'center' }); }
  } catch (e) { empty(errorMessage(e)); }
}
boot();
