// Injected only into the pinned baseline's benchmark build. Observe the real
// view and decoded images; never replace paint(), layout(), or event handlers.
let creditiaAcceptanceRaw: any = null;
function creditiaAcceptanceApi(cmd: string, args: any) {
  switch (cmd) {
    case 'projects': return [{ id: 'fixture', name: creditiaAcceptanceRaw.project.name, path: '', exists: false, hasCredits: true }];
    case 'credits': return creditiaAcceptanceRaw;
    case 'stamp': return 'acceptance-fixture';
    case 'head': return creditiaAcceptanceRaw.head ?? '';
    case 'config_of': return {};
    case 'logo': return DEFAULT_LOGO;
    default: throw Error(`Unexpected acceptance fixture command: ${cmd}`);
  }
}
async function creditiaAcceptanceStart() {
  if (!native) throw Error('The baseline benchmark requires the original native Tauri app');
  creditiaAcceptanceRaw = await native('acceptance_fixture');
  // CI assigns a fresh XDG data directory for each cold session; clear only
  // Creditia's namespaced preferences in this isolated benchmark profile.
  for (const key of Object.keys(localStorage)) if (key.startsWith('creditia.')) localStorage.removeItem(key);
  for (const [key, value] of Object.entries({ theme: 'minimal-light', lang: 'en', names: 'off', view: 'wall', autofresh: 'off' })) store.set(key, value);
  state.style = kitById('minimal-light')!; state.lang = 'en'; state.size = 'native';
  let previous = '', ready = false, seq = 0, queued = false;
  const visible = (el: Element) => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0 && r.bottom > 104 && r.top < innerHeight && r.right > 0 && r.left < innerWidth; };
  const decoded = (el: Element) => {
    const content = [...el.querySelectorAll('img,canvas,svg')];
    return content.length > 0 && content.every(child => child instanceof HTMLImageElement ? child.complete && child.naturalWidth > 0 : true);
  };
  const inspect = () => {
    queued = false;
    const cards = [...document.querySelectorAll('#board .tile')].filter(visible);
    const focus = document.querySelector<HTMLElement>('#focus');
    const focused = !!focus && !focus.hidden;
    const valid = !!state.raw && cards.length > 0 && cards.every(card => !card.classList.contains('slot') && decoded(card.querySelector('.art') ?? card));
    const snapshot = { view: view(), focused, group: groupPref(), named: namesOn(), zoom: 1,
      viewport_x: roam.x, viewport_y: view() === 'roam' ? roam.y : ($('#stage')?.scrollTop ?? 0),
      tile_count: tiles().length, style: state.style.id, playing: false,
      visible: !document.hidden, focus_ready: focused && !!focus?.querySelector('.big:not(.loading) .art') && decoded(focus.querySelector('.big .art')!),
      width: innerWidth, height: innerHeight, ready: ready || valid,
      first_tile: (() => { const art = document.querySelector('#board .tile:not(.slot) .art'); if (!art) return null; const r = art.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + Math.min(r.height / 2, 100) }; })() };
    const encoded = JSON.stringify(snapshot);
    if (encoded !== previous) { previous = encoded; void native('acceptance_event', { event: { event: !ready && valid ? 'ready' : 'state', sequence: ++seq, elapsed_ms: performance.now(), state: snapshot, width: innerWidth, height: innerHeight, tiles: tiles().length } }); }
    if (valid) ready = true;
  };
  const schedule = () => { if (!queued) { queued = true; requestAnimationFrame(inspect); } };
  // Mutation and actual input observations avoid a periodic idle polling load.
  new MutationObserver(schedule).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['style', 'class', 'hidden'] });
  for (const event of ['scroll', 'wheel', 'keydown', 'keyup', 'pointerup', 'load', 'resize']) addEventListener(event, schedule, true);
  document.addEventListener('visibilitychange', inspect); // rAF is paused while hidden.
  await boot(); schedule();
}
