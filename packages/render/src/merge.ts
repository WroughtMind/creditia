// 跨项目汇总：把几个项目的档案并成一份，格式和单个项目一样，所以每种作品都能直接画「全部项目」。
// 加得起来的（行数、按天、按周、作息、会话、证据等级、板块）相加；版本是各项目自己的，不合并；
// 文件路径、项（PR / 提交）前面加上项目名，免得不同项目的同名文件、同号 PR 混在一起。

export interface Part { name: string; raw: any }

const num = (x: unknown) => (typeof x === 'number' && isFinite(x) ? x : 0);
/** 两个只含数字（可以嵌套）的对象相加 */
function addInto(dst: any, src: any) {
  for (const [k, v] of Object.entries(src ?? {})) {
    if (typeof v === 'number') dst[k] = num(dst[k]) + v;
    else if (v && typeof v === 'object' && !Array.isArray(v)) addInto((dst[k] ??= {}), v);
  }
  return dst;
}

export function mergeCredits(parts: Part[], lang: 'zh' | 'en' = 'zh'): any {
  const ps = parts.filter((p) => p.raw && typeof p.raw === 'object');
  const contributors = new Map<string, any>();
  const breakdown = new Map<string, any>();
  const weeks = new Map<string, any>(), days = new Map<string, any>();
  const coverage = new Map<string, any>();
  const clock: number[][] = Array.from({ length: 7 }, () => Array(24).fill(0));
  const totals: any = {}, human: any = {};
  const files: any[] = [], sessions: any[] = [], units: any[] = [], directors: any[] = [];
  let from = '', to = '', conflictLines = 0, generatedAt = '';
  for (const { name, raw } of ps) {
    for (const c of raw.contributors ?? []) {
      const e = contributors.get(c.id);
      if (!e) contributors.set(c.id, structuredClone(c));
      else e.roles = [...new Set([...(e.roles ?? []), ...(c.roles ?? [])])];
    }
    for (const b of raw.breakdown ?? []) {
      const { contributor, byVersion: _v, ...rest } = b;
      addInto((breakdown.get(contributor) ?? breakdown.set(contributor, { contributor }).get(contributor)), rest);
    }
    for (const w of raw.timeline ?? []) addInto((weeks.get(w.week) ?? weeks.set(w.week, { week: w.week, byContributor: {} }).get(w.week)).byContributor, w.byContributor);
    for (const d of raw.days ?? []) addInto((days.get(d.date) ?? days.set(d.date, { date: d.date, by: {} }).get(d.date)).by, d.by);
    for (const cv of raw.coverage ?? []) {
      const e = coverage.get(cv.name);
      if (!e) coverage.set(cv.name, { ...cv, _n: 1 });
      else {
        e.from = e.from < cv.from ? e.from : cv.from; e.to = e.to > cv.to ? e.to : cv.to;
        if (cv.best === 'verified') e.best = 'verified';
        e.verifiedShare = (e.verifiedShare * e._n + num(cv.verifiedShare)) / (e._n + 1); e._n++;
      }
    }
    (raw.clock ?? []).forEach((row: number[], i: number) => row?.forEach((v, j) => { if (clock[i]) clock[i][j] += num(v); }));
    addInto(totals, raw.totals);
    addInto(human, raw.human);
    for (const f of raw.files ?? []) files.push(f.path ? { ...f, path: `${name}/${f.path}` } : f);
    sessions.push(...(raw.sessions ?? []));
    for (const u of raw.units ?? []) units.push({ ...u, id: `${name}:${u.id}`, title: u.title ? `${name} · ${u.title}` : name, version: undefined });
    directors.push(...(raw.directors ?? []));
    const r = raw.project?.range ?? {};
    if (r.from && (!from || r.from < from)) from = r.from;
    if (r.to && (!to || r.to > to)) to = r.to;
    conflictLines += num(raw.conflictLines);
    if ((raw.generatedAt ?? '') > generatedAt) generatedAt = raw.generatedAt;
  }
  sessions.sort((a, b) => num(a.start) - num(b.start));
  const dayList = [...days.values()].sort((a, b) => a.date.localeCompare(b.date));
  totals.days = dayList.length;
  totals.units = units.length;
  return {
    schemaVersion: 0,
    project: { name: lang === 'zh' ? '全部项目' : 'All projects', repos: [], range: { from, to } },
    /** 汇总了哪些项目：各自的名字、时间、行数 */
    projects: ps.map(({ name, raw }) => ({ name, range: raw.project?.range ?? {}, linesAdded: num(raw.totals?.linesAdded), commits: num(raw.totals?.commits) })),
    generatedAt,
    level: ps[0]?.raw.level,
    versions: [],
    coverage: [...coverage.values()].map(({ _n, ...c }) => c).sort((a, b) => String(a.from).localeCompare(String(b.from))),
    directors,
    contributors: [...contributors.values()],
    totals,
    breakdown: [...breakdown.values()].sort((a, b) => num(b.linesAdded) - num(a.linesAdded)),
    human,
    timeline: [...weeks.values()].sort((a, b) => a.week.localeCompare(b.week, 'en', { numeric: true })),
    days: dayList,
    clock,
    files: files.sort((a, b) => num(b.lines) - num(a.lines)),
    sessions,
    units: units.sort((a, b) => String(a.date).localeCompare(String(b.date))),
    conflictLines,
  };
}
