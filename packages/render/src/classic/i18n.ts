// 原版作品的英文：原版的文字是写在各形式里的中文。出图前把作品树里的文字按这张表换成英文，
// 形式本身不动。固定的词直接对照；带数字的说法用规则换。表里没有的保持原样。

const WORDS: Record<string, string> = {
  主笔: 'Lead', 协作: 'With', 审阅: 'Review', 参与: 'With', 主导: 'Lead',
  署名: 'Credits', 致谢: 'Credits', '‹ 关于': '‹ About', 合计: 'Total', '合计 TOTAL': 'TOTAL', 'Total 合计': 'Total',
  工期: 'Span', 动工: 'Active', 会话: 'Sessions', 提交: 'Commits', 写下: 'Written', 留下: 'Kept', 至今仍在: 'Still here',
  剧终: 'The End', 全剧: 'Full run', 未发布: 'Unreleased', 今晚演出: 'Tonight', 次提交: 'commits', 行: 'lines', 代码: 'Code',
  其他: 'Others', 未确认: 'Unattributed',
  'cast 演职员': 'cast', 'colophon 版权页': 'colophon', 'credits 署名': 'credits', 'ENERGY RECEIPT 精力小票': 'ENERGY RECEIPT',
  'ledger 账簿': 'ledger', 'OPEN 24H · 署名': 'OPEN 24H · CREDITS', 'PLAYBILL 节目单': 'PLAYBILL', 'programme 节目单': 'programme',
  'THANK YOU · 请保留小票': 'THANK YOU · PLEASE KEEP', 'Creditia — 作品署名': 'Creditia — credits', 'Creditia — 命令行': 'Creditia — terminal',
  'Creditia · 精力小票': 'Creditia · receipt', '工作记录 · 精力清单': 'Work log', '一格 = 1%': '1 square = 1%',
  各版本新增代码: 'Lines added per release', 每周新增代码: 'Lines added per week', '# 写下的代码留下多少': '# how much written code stayed',
  '> 统计': '> stats', '由 creditia 开具': 'issued by creditia', 留到了今天: 'still here today',
  有提交的天数: 'Days with commits', 发给AI的消息: 'Messages to AI', 'AI 写的': 'Written by AI', 提交了的: 'Committed', 现在还在: 'Still here',
};
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const WEEK: Record<string, string> = { 日: 'Sun', 一: 'Mon', 二: 'Tue', 三: 'Wed', 四: 'Thu', 五: 'Fri', 六: 'Sat' };
const enList = (s: string) => s.replace(/、/g, ', ').replace(/ 和 /g, ' and ');

const RULES: [RegExp, string | ((...a: string[]) => string)][] = [
  [/^周(.)$/, (_, d) => WEEK[d] ?? d],
  [/^[一二三四五六日]$/, (d) => WEEK[d] ?? d],
  [/(\d{4}) 年 (\d+) 月起/, (_, y, mo) => `since ${MONTHS[+mo - 1]} ${y}`],
  [/(\d+)月(\d+)日/, (_, mo, d) => `${MONTHS[+mo - 1]} ${d}`],
  [/(\d+)月/, (_, mo) => MONTHS[+mo - 1] ?? mo],
  [/(\d+)时/, '$1h'],
  [/([\d,]+) 个文件 · 现存 ([\d,.KMB]+) 行/, '$1 files · $2 lines still here'],
  [/([\d,.KMB]+) 行至今仍在/, '$1 lines still here'],
  [/([\d,.KMB]+) 行，留下 ([\d,.KMB]+)/, '$1 lines, $2 kept'],
  [/行 · ([\d,]+) 个作品/, 'lines · $1 projects'],
  [/([\d,]+) 次提交/g, '$1 commits'],
  [/([\d,]+) 次会话/g, '$1 sessions'],
  [/([\d,]+) 项/g, '$1 units'],
  [/([\d,]+) 天/g, '$1 days'],
  [/([\d,.]+[KMB]?) 行/g, '$1 lines'],
  [/^(\S+) 格$/, '$1 squares'],
  [/格是 (.+)$/, 'squares are $1'],
  [/^与 (.+) 共同完成$/, (_, l) => `Made with ${enList(l)}`],
  [/^(.+) 与 (.+) 共同完成$/, (_, w, l) => `${w} with ${enList(l)}`],
  [/由 creditia 开具 · (.+)/, 'issued by creditia · $1'],
  [/creditia 记 · /, 'creditia · '],
  [/^(.+) 作品$/, '$1 projects'],
];

const CJK = /[一-鿿、，：；（）「」]/;
/** 拼在长句里的词：长的先换，免得「每周新增代码」里的「代码」先被换掉 */
const BY_LENGTH = Object.entries(WORDS).filter(([zh]) => zh.length >= 2).sort((a, b) => b[0].length - a[0].length);
export function enText(s: string): string {
  if (!CJK.test(s)) return s;
  const t = s.trim();
  if (WORDS[t] !== undefined) return s.replace(t, WORDS[t]);
  let out = s;
  for (const [re, to] of RULES) out = out.replace(re, to as any);
  // 拼起来的短语（「主笔 Claude Code · 协作 Codex」这类）：把里面认得的词逐个换掉
  if (CJK.test(out)) for (const [zh, en] of BY_LENGTH) if (out.includes(zh)) out = out.split(zh).join(en);
  // 中文标点换成英文的
  return out.replace(/、/g, ', ').replace(/，/g, ', ').replace(/：/g, ': ').replace(/；/g, '; ').replace(/（/g, ' (').replace(/）/g, ')').replace(/[「」]/g, '"');
}

/** 把作品树里的文字换成英文（原地改） */
export function enTree(n: any): any {
  if (!n || typeof n !== 'object') return n;
  const kids = n.props?.children;
  if (typeof kids === 'string') n.props.children = enText(kids);
  else if (Array.isArray(kids)) n.props.children = kids.map((k) => (typeof k === 'string' ? enText(k) : enTree(k)));
  else if (kids && typeof kids === 'object') enTree(kids);
  return n;
}

/** SVG 字符串里的文字（徽章是直接拼的 SVG） */
export const enSvg = (svg: string) => svg.replace(/>([^<>]*[一-鿿、，：][^<>]*)</g, (_, s) => `>${enText(s)}<`);

/** 树里剩下的中文（测试用） */
export function leftoverCjk(n: any, out: string[] = []): string[] {
  if (!n || typeof n !== 'object') return out;
  const kids = n.props?.children;
  for (const k of Array.isArray(kids) ? kids : [kids]) {
    if (typeof k === 'string') { if (CJK.test(k)) out.push(k); } else leftoverCjk(k, out);
  }
  return out;
}
