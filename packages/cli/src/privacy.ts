// 公开前的隐私检查：credits.json 里会公开哪些字段（列出几个样例），以及看起来不该公开的东西（密钥、邮箱、本机路径、内网地址）。
// 只看档案里本来就有的文字，不读代码；按类别隐藏只改要公开的这一份，不动本机档案。

export type Cat = 'titles' | 'paths' | 'people' | 'hours' | 'repos';
export const CATS: Cat[] = ['titles', 'paths', 'people', 'hours', 'repos'];

export interface Risk { cat: Cat; kind: string; text: string }
export interface Report {
  project: string;
  /** 每类有几条、前几条样例 */
  cats: Record<Cat, { n: number; samples: string[] }>;
  risks: Risk[];
}

const PATTERNS: [string, RegExp][] = [
  ['key', /\b(?:sk-(?:ant-|proj-)?[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[abprs]-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{30,}|glpat-[A-Za-z0-9_-]{16,})/],
  ['key', /-----BEGIN [A-Z ]*PRIVATE KEY-----/],
  ['credential', /[a-z][a-z0-9+.-]*:\/\/[^\s/:@]+:[^\s/@]+@/i],
  ['email', /\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b/],
  ['local-path', /(?:^|[\s"'(])(?:\/Users\/|\/home\/|\/root\/|[A-Za-z]:\\Users\\)[^\s"')]+/],
  ['internal-host', /\b(?:localhost|[a-z0-9-]+\.(?:local|internal|corp|lan|intranet))\b|\b(?:10\.\d{1,3}|192\.168|172\.(?:1[6-9]|2\d|3[01]))\.\d{1,3}\.\d{1,3}\b/i],
];

/** 隐藏标题时的写法：和核心 hide_titles 一致 */
const isPlaceholder = (u: any) => /^PR #\d+$/.test(u.title ?? '') || /^[0-9a-f]{7}$/.test(u.title ?? '');

export function check(raw: any): Report {
  const titles = (raw.units ?? []).filter((u: any) => u.title && !isPlaceholder(u)).map((u: any) => String(u.title));
  const paths = (raw.files ?? []).filter((f: any) => f.path).map((f: any) => String(f.path));
  const people = [...new Set([...(raw.contributors ?? []).filter((c: any) => c.kind === 'human'), ...(raw.authors ?? [])].filter((c: any) => c.name).map((c: any) => String(c.name)))] as string[];
  const hours = (raw.sessions ?? []).length + ((raw.clock ?? []).flat().some((x: number) => x > 0) ? 1 : 0);
  const repos = (raw.project?.repos ?? []).map(String);
  const cats: Report['cats'] = {
    titles: { n: titles.length, samples: titles.slice(0, 3) },
    paths: { n: paths.length, samples: paths.slice(0, 3) },
    people: { n: people.length, samples: people.slice(0, 5) },
    hours: { n: hours, samples: [] },
    repos: { n: repos.length, samples: repos.slice(0, 3) },
  };
  const risks: Risk[] = [];
  const scan = (cat: Cat, list: string[]) => {
    for (const text of list) for (const [kind, re] of PATTERNS) if (re.test(text)) {
      // 密钥、密码不原样印出来
      const shown = kind === 'key' || kind === 'credential' ? text.replace(new RegExp(re.source, re.flags.replace('g', '') + 'g'), (x) => x.slice(0, 6) + '…') : text;
      risks.push({ cat, kind, text: shown.length > 120 ? shown.slice(0, 117) + '…' : shown });
      break;
    }
  };
  scan('titles', titles); scan('paths', paths); scan('people', people); scan('repos', repos);
  scan('titles', [String(raw.project?.name ?? '')]);
  return { project: String(raw.project?.name ?? ''), cats, risks };
}

/** 要公开的那一份：按类别隐藏。仓库地址里的账号密码总是去掉 */
export function redact(raw: any, hide: Set<Cat>): any {
  const out = structuredClone(raw);
  if (hide.has('titles')) for (const u of out.units ?? []) u.title = String(u.id ?? '').startsWith('pr:') ? `PR #${String(u.id).slice(3)}` : String(u.commits?.[0] ?? '').slice(0, 7);
  if (hide.has('paths')) for (const f of out.files ?? []) delete f.path;
  if (hide.has('people')) {
    for (const c of out.contributors ?? []) if (c.kind === 'human') delete c.name;
    for (const a of out.authors ?? []) delete a.name;
  }
  if (hide.has('hours')) { delete out.sessions; delete out.clock; }
  if (out.project) {
    if (hide.has('repos')) delete out.project.repos;
    else if (Array.isArray(out.project.repos)) out.project.repos = out.project.repos.map((r: string) => String(r).replace(/(\/\/)[^/@\s]+@/, '$1'));
  }
  return out;
}

/** 文字：中英文 */
export const CAT_NAMES: Record<Cat, [string, string]> = {
  titles: ['PR / 提交标题', 'PR / commit titles'],
  paths: ['文件路径', 'file paths'],
  people: ['人名', "people's names"],
  hours: ['工作时间（会话起止、作息图）', 'working hours (session times, punch card)'],
  repos: ['仓库地址', 'repository URLs'],
};
export const RISK_NAMES: Record<string, [string, string]> = {
  key: ['像是密钥', 'looks like a secret key'],
  credential: ['地址里带账号密码', 'URL with credentials'],
  email: ['邮箱', 'email address'],
  'local-path': ['本机路径', 'local path'],
  'internal-host': ['内网地址', 'internal host'],
};
