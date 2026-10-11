import { api, type Project } from './api.ts';
import { $, esc, icon, toast, copy } from './dom.ts';
import { fmt, pct, type Model } from '../../../packages/render/src/data.ts';
import { creditsBlock } from '../../../packages/render/src/text.ts';
export interface PanelController {
  state: { projects: Project[]; current: Project | null; raw: any; config: any; model: Model | null };
  openProject: (project: Project) => Promise<void>;
  paintWorkspace: () => Promise<void>;
  rebuildModel: () => void;
  empty: (message?: string, path?: string) => void;
}
const message = (error: unknown) => error instanceof Error ? error.message : String(error);
export function showPanel(title: string, content: string, className = '') {
  const panel = $<HTMLDialogElement>('#panel'); if (panel.open) panel.close(); panel.className = `panel glass ${className}`;
  panel.innerHTML = `<div class="panel-heading"><h2>${esc(title)}</h2><button class="icon-button" aria-label="关闭">${icon('close')}</button></div><div class="panel-content">${content}</div>`;
  const actions = [...panel.querySelectorAll<HTMLButtonElement>('.primary')];
  if (actions.length) {
    const footer = document.createElement('div'); footer.className = 'panel-footer';
    actions.forEach(button => { const form = button.closest('form'); if (form?.id) button.setAttribute('form', form.id); footer.append(button); });
    panel.append(footer);
  }
  panel.querySelector('button')!.onclick = closePanel; panel.showModal(); return panel;
}
export function closePanel() { $<HTMLDialogElement>('#panel').close(); }
export function initPanels(controller: PanelController) {
  const { state, openProject, paintWorkspace, rebuildModel } = controller;
  let analyzing = false;
  function projectMenu() {
    const dialog = $<HTMLDialogElement>('#project-menu');
    dialog.innerHTML = `<input id="project-search" type="search" placeholder="搜索项目" aria-label="搜索项目"><div id="project-list"></div><button id="add-project" class="menu-add">${icon('plus')} 打开项目</button>`;
    const render = () => { const query = $<HTMLInputElement>('#project-search').value.toLowerCase(); $('#project-list').innerHTML = state.projects.filter(p => `${p.name} ${p.path}`.toLowerCase().includes(query)).map(p => `<button class="project-row ${p.id === state.current?.id ? 'active' : ''}" data-project="${esc(p.id)}"><b>${esc(p.name)}</b><span>${esc(p.path)}</span></button>`).join(''); };
    render(); $('#project-search').oninput = render;
    dialog.onclick = e => { const row = (e.target as HTMLElement).closest<HTMLElement>('[data-project]'); if (row) { dialog.close(); openProject(state.projects.find(p => p.id === row.dataset.project)!); } };
    $('#add-project').onclick = () => { dialog.close(); addProject(); }; dialog.showModal();
  }
  function addProject(path = '') {
    const panel = showPanel('打开项目', `<p>读取本机 Git 项目的提交与编程工具记录。</p><form id="project-form"><label>项目文件夹<input id="project-path" value="${esc(path)}" placeholder="粘贴项目的完整路径" required></label><p class="field-error" id="project-error" hidden></p><button class="primary" type="submit">开始分析</button></form>`);
    panel.querySelector('form')!.onsubmit = async e => {
      e.preventDefault(); const button = panel.querySelector<HTMLButtonElement>('[type="submit"]')!; button.disabled = true; button.textContent = '正在读取项目记录…';
      try { const result = await api('analyze_project', { path: $<HTMLInputElement>('#project-path').value.trim() }); state.projects = await api('projects'); if (!button.isConnected || !panel.open) { toast('项目分析已完成，可从顶部切换'); return; } closePanel(); await openProject(state.projects.find(p => p.id === result.id)!); if (result.warnings?.length) toast(result.warnings.join('；'), true); }
      catch (e) { const error = panel.querySelector<HTMLElement>('#project-error'); if (button.isConnected && error) { error.textContent = message(e); error.hidden = false; } }
      finally { button.disabled = false; button.textContent = '开始分析'; }
    };
  }
  async function refresh() {
    if (!state.current || analyzing) return;
    const project = state.current; analyzing = true; $<HTMLButtonElement>('#refresh-button').disabled = true; toast('正在读取新的提交与工具记录…', true);
    try { const result = await api('analyze_project', { path: project.path, branch: state.raw?.branch }); state.projects = await api('projects'); if (state.current?.id === project.id) await openProject(state.projects.find(p => p.id === result.id)!); toast(result.warnings?.length ? result.warnings.join('；') : '项目记录已更新'); }
    catch (e) { toast(message(e), true); } finally { analyzing = false; $<HTMLButtonElement>('#refresh-button').disabled = false; }
  }
  function peoplePanel() {
    if (!state.model) return;
    const project = state.current!, people = state.config.people ?? state.raw.people ?? [];
    const panel = showPanel('制作人员', `<p>写下参与者做过的事。姓名和角色作为独立署名，不参与模型排行。</p><label class="check-row"><input id="show-people" type="checkbox" ${state.config.display?.people ? 'checked' : ''}>在作品中显示署名</label><form id="people-form"><div id="people-list"></div><button id="add-person" type="button" class="text-button">${icon('plus',16)} 添加一位</button><p class="field-error" id="people-error" hidden></p><button class="primary" type="submit">保存署名</button></form>`);
    const row = (p: any = {}) => { const div = document.createElement('div'); div.className = 'person-row'; div.innerHTML = `<div><input class="person-name" placeholder="姓名或署名" aria-label="姓名" value="${esc(p.name)}"><button type="button" class="icon-button" aria-label="移除此署名">${icon('close',14)}</button></div><input class="person-roles" placeholder="角色，例如：构思、设计、开发、审阅" aria-label="角色" value="${esc((p.roles ?? []).join('、'))}"><textarea class="person-note" placeholder="一段想留下的话（可选）" aria-label="投入说明">${esc(p.note)}</textarea>`; div.querySelector('button')!.onclick = () => div.remove(); $('#people-list').append(div); };
    people.forEach(row); if (!people.length) row(); $('#add-person').onclick = () => row();
    panel.querySelector('form')!.onsubmit = async e => {
      e.preventDefault(); const button = panel.querySelector<HTMLButtonElement>('[type="submit"]')!; button.disabled = true;
      const people = [...panel.querySelectorAll<HTMLElement>('.person-row')].map(r => ({ name: r.querySelector<HTMLInputElement>('.person-name')!.value.trim(), roles: r.querySelector<HTMLInputElement>('.person-roles')!.value.split(/[,，、]/).map(s => s.trim()).filter(Boolean), note: r.querySelector<HTMLTextAreaElement>('.person-note')!.value.trim() })).filter(p => p.name);
      const show = $<HTMLInputElement>('#show-people').checked;
      try { await api('set_people', { projectId: project.id, people, show }); if (state.current?.id === project.id) { state.config.people = people; state.config.display = { ...state.config.display, people: show }; rebuildModel(); if (button.isConnected && panel.open) closePanel(); await paintWorkspace(); } toast('署名已保存'); }
      catch (e) { const error = panel.querySelector<HTMLElement>('#people-error'); if (button.isConnected && error) { error.hidden = false; error.textContent = message(e); } } finally { button.disabled = false; }
    };
  }
  async function archivePanel() {
    if (!state.model || !state.current) return;
    const model = state.model, raw = state.raw, project = state.current;
    const evidence = new Map<string, number>(); for (const row of raw.breakdown ?? []) for (const [key, v] of Object.entries(row.byEvidence ?? {})) evidence.set(key, (evidence.get(key) ?? 0) + ((v as any).linesAdded ?? 0));
    const labels: Record<string,string> = { verified: '逐行记录', declared: '提交声明', inferred: '推测', manual: '人工确认', unattributed: '未确认' };
    const panel = showPanel('项目记录', `<p class="record-path">${esc(project.path)}</p><p>${raw.archive?.historical ? '已保存的历史档案 · ' : ''}分支 ${esc(raw.branch ?? '未记录')} · ${esc(raw.generatedAt?.slice(0,10) ?? '')}</p><div class="record-summary"><div><b>${fmt(model.linesAdded)}</b><span>历史新增代码行</span></div><div><b>${fmt(model.linesSurviving)}</b><span>当前保留代码行</span></div><div><b>${fmt(model.sessions)}</b><span>记录到的会话</span></div></div><p>数字描述已记录的工作，不衡量项目价值，也不折算人的劳动时间。时间按 ${esc(raw.measurement?.timezone ?? '档案原有时区')} 展示。</p><div class="evidence-list">${[...evidence].map(([key,n]) => `<div><span>${esc(labels[key] ?? key)}</span><b>${pct(model.linesAdded ? n / model.linesAdded * 100 : 0)}</b></div>`).join('')}</div><details><summary>项目的其他路径</summary><p>工作树会自动关联；移动或复制的项目可在此关联旧路径。</p><div id="linked-paths">${[...new Set([...(project.paths ?? []), ...(state.config.paths ?? [])])].filter(p => p !== project.path).map((p: string) => `<p class="record-path">${esc(p)}</p>`).join('')}</div><form id="link-form"><input id="link-path" placeholder="其他工作副本或旧路径" required><button type="submit">关联路径</button></form><p id="link-result"></p></details><details><summary>数据源与读取情况</summary><div id="health">正在读取…</div></details><details><summary>提交记录 · ${raw.units?.length ?? 0}</summary><div class="units">${[...(raw.units ?? [])].sort((a,b) => b.date.localeCompare(a.date)).map(u => `<div><time>${esc(u.date)}</time><span>${esc(u.title)}</span></div>`).join('')}</div></details><button class="text-button" id="copy-record">${icon('copy',16)} 复制署名文档</button>`, 'record-panel');
    $('#copy-record').onclick = () => copy(creditsBlock(model)).catch(e => toast(message(e)));
    panel.querySelector('form')!.onsubmit = async e => {
      e.preventDefault(); const path = $<HTMLInputElement>('#link-path').value.trim(); const result = panel.querySelector<HTMLElement>('#link-result')!;
      try { await api('link_path', { projectId: project.id, path }); if (state.current?.id === project.id) state.config.paths = [...new Set([...(state.config.paths ?? []), path])]; if (!result.isConnected) return; panel.querySelector('#linked-paths')!.insertAdjacentHTML('beforeend', `<p class="record-path">${esc(path)}</p>`); result.textContent = '已关联，重新分析时读取此路径的记录。'; } catch (e) { result.textContent = message(e); }
    };
    const healthNode = panel.querySelector('#health')!;
    try { const health = await api('doctor'); if (!healthNode.isConnected) return; healthNode.innerHTML = health.map((h: any) => `<div class="health-row"><b>${esc(h.source)}</b><span>${h.readable ? `${fmt(h.records)} 条记录` : esc(h.note || '未找到记录')}</span></div>`).join(''); } catch (e) { healthNode.textContent = message(e); }
  }
  return { projectMenu, addProject, refresh, peoplePanel, archivePanel };
}
