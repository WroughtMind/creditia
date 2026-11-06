// Human-readable views of the validated result. Never change gate decisions here.
import fs from 'node:fs';
import path from 'node:path';

const cell = v => String(v ?? '').replace(/[\t\r\n]+/g, ' ');
const unique = xs => [...new Set(xs.filter(Boolean))].join(';');
const tsv = rows => rows.map(row => row.map(cell).join('\t')).join('\n') + '\n';
function indexCatalog(catalog) {
  const result = new Map();
  for (const m of catalog?.manifests ?? []) for (const c of m.cases) {
    const rows = result.get(c.row) ?? [];
    rows.push({...c, manifest: m.manifest}); result.set(c.row, rows);
  }
  return result;
}
function catalogStatus(cases) {
  if (cases.some(c => c.status === 'failed')) return 'failed';
  if (cases.some(c => c.status === 'passed')) return 'passed';
  return cases[0]?.status ?? 'not_run';
}
const platforms = [['darwin-arm64','macOS Apple Silicon'],['darwin-x64','macOS Intel'],['win32-x64','Windows x64'],['linux-x64','Linux x64']];
const scenarios = {startup:'启动',scroll:'滚动',roam:'漫游',zoom:'缩放',idle:'空闲', 'hidden-idle':'隐藏后空闲'};
const pair = (v, unit, decimals) => v?.length === 2 && v.every(Number.isFinite) ? v.map(n => (n/unit).toFixed(decimals)).join(' → ') : '待测';
export function writeDeliveryReport(r, output, {matrix, packages = [], performance = []}) {
  const native = indexCatalog(r.native_catalog), browser = indexCatalog(r.browser_catalog);
  const scope = new Map(matrix.map(row => [[row.renderer,row.style,row.piece,row.variant].join('/'),row]));
  const art = [['renderer','style','piece','layout','desktop','shared_render','preference_matches','all_preferences','motion','exports','web','web_manifest','native','native_screenshot_sha256','native_manifest','renderer_manifests','candidate_sha']];
  for (const row of r.artwork) {
    const b = browser.get(row.key) ?? [], n = native.get(row.key) ?? [];
    const animated = scope.get(row.key)?.animated === 'true';
    art.push([...row.key.split('/'),row.desktop,row.status,unique(row.preference_coverage),row.all_preferences,animated ? row.motion_sampled ? 'sampled' : 'not_verified' : 'not_applicable',row.exports_checked ? 'checked' : 'not_verified',catalogStatus(b),unique(b.map(c=>c.manifest)),row.desktop ? catalogStatus(n) : 'shared_only',unique(n.map(c=>c.screenshot_sha256)),unique(n.map(c=>c.manifest)),unique(row.evidence.map(e=>e.manifest)),r.candidate_sha]);
  }
  const checks = [['feature_id','feature','check','status','required_tests','evidence','combined_test_sources','recorded_review','candidate_sha']];
  for (const f of r.feature_coverage) for (const c of f.checks) checks.push([f.id,f.name,c.name,c.status,unique(c.required_tests),unique(c.evidence),c.combined_test_sources,c.recorded_review,r.candidate_sha]);
  const checksPassed = checks.slice(1).filter(row=>row[3]==='passed').length;
  fs.writeFileSync(path.join(output,'ARTWORK-STATUS.tsv'),tsv(art));
  fs.writeFileSync(path.join(output,'FEATURE-STATUS.tsv'),tsv(checks));
  const desktop = r.artwork.filter(a=>a.desktop).length, perf = new Map(performance.map(p=>[p.manifest,p]));
  const installed = packages.filter(p=>r.package_platforms.some(item=>item.manifest===p.manifest && item.installed_workflow));
  const ready = r.release_ready === true;
  const doc = ['# Creditia GPUI 迁移验收','',`实测提交：\`${r.candidate_sha}\`。**${ready?'已达到切换条件':'尚未达到切换条件'}**。`,
    '', '桌面界面采用 GPUI。项目库和分析继续使用 Rust 服务，展墙、漫游、动画和导出复用 CLI 与网页的作品渲染器。安装包内含运行时与字体。',
    '', '| 验收项 | 本次证据 |','|---|---:|',
    `| 全目录按需回归 | ${r.artwork.filter(a=>a.status==='machine_matched'&&a.all_preferences&&a.motion_sampled&&a.exports_checked).length} / ${r.artwork.length} 组完成偏好、适用动作及导出验证 |`,
    `| 网页作品 | ${r.browser_catalog.passed_rows} / ${r.artwork.length} 组 |`,
    `| 原生窗口作品 | ${r.native_catalog.passed_rows} / ${desktop} 组 |`,
    `| 功能操作 | ${checksPassed} / ${checks.length-1} 项；${r.feature_coverage.filter(f=>f.complete).length} / ${r.feature_coverage.length} 组全部通过 |`,
    `| 原生交互用例 | ${r.native.reduce((s,m)=>s+m.cases.filter(c=>c.status==='passed').length,0)} 项通过；四种目标平台${r.native_platforms_complete?'均有记录':'记录不齐'} |`,
    `| 实际安装 | ${platforms.filter(([id])=>installed.some(p=>p.platform===id)).length} / 4 平台 |`,
    `| 同机性能对照 | ${r.performance_comparisons.filter(p=>p.status==='comparable').length} 对 |`,
    `| 视觉复核 | ${r.recorded_review.status}；已记录 ${r.recorded_review.reviewed_artworks} 个代表组合 |`,
    `| 无效证据 | ${r.invalid_evidence.length} 份 |`,
    '', '## 主平台使用记录', '',
    ...(r.recorded_review.workflow ?? []).map(entry => `- ${entry.notes}`),
    ...(r.recorded_review.workflow?.length ? [] : ['等待主平台验收。']),
    '', '## 已知限制与回退', '',
    'macOS 候选包使用临时签名，Windows 候选包使用未签名安装程序。同机性能只代表所测硬件和显示环境。',
    '升级前保留上一版应用和项目库备份。需要回退时退出 GPUI，重新打开上一版，保留项目的 .creditia 目录和证据库。具体步骤见 INSTALL.md。',
    '', '[逐作品、版式、风格台账](ARTWORK-STATUS.tsv) · [逐项操作台账](FEATURE-STATUS.tsv) · [完整证据索引](report.json)',
    '', '共享目录中只有标记为 desktop 的作品需要 GPUI 窗口截图。动作栏的 sampled 表示导出采样已核对；窗口播放、中断和重播另见功能台账。',
    '', '## 安装包','', '| 平台 | 安装并核对的文件 | SHA-256 |','|---|---|---|',
    ...platforms.flatMap(([id,label])=>{const files=installed.filter(p=>p.platform===id).flatMap(p=>p.installers);return files.length ? files.map(f=>`| ${label} | ${f.name} | \`${f.sha256}\` |`) : [`| ${label} | 待验收 | — |`];}),
    '', '## 同设备性能','', '各数值按旧版 Tauri → GPUI 排列。RSS 为整组进程峰值；CPU 为采样中位数，100% 表示占满一个逻辑核心。启动耗时取启动场景中应用报告的就绪时间。', '',
    '| 场景 | 缓存 | 峰值 RSS（MiB） | CPU（%） | 启动耗时（秒） |','|---|---|---:|---:|---:|',
    ...r.performance_comparisons.map(p=>`| ${scenarios[p.scenario]??p.scenario} | ${p.cache_state==='cold'?'冷':'热'} | ${pair(p.rss_peak_bytes,1048576,1)} | ${pair(p.cpu_p50_percent_one_core,1,1)} | ${p.scenario==='startup'?pair([p.baseline,p.candidate].map(k=>perf.get(k)?.phase_summary?.startup?.duration_seconds),1,2):'—'} |`),
    ...(r.performance_comparisons.length ? [] : ['| 待测 | — | — | — | — |']),
    '',...(performance.some(p=>p.provenance?.environment?.rendering?.LIBGL_ALWAYS_SOFTWARE==='1') ? ['本轮含 Linux 软件渲染测量。数据只代表所测设备与场景，不能据此推断其他平台的硬件 GPU 占用。',''] : []),
    '失败或未执行的项目保留在台账中；原生截图、安装记录、导出和动作证据均可从 JSON 索引追溯。',''];
  fs.writeFileSync(path.join(output,'ACCEPTANCE.zh.md'),doc.join('\n'));
}
