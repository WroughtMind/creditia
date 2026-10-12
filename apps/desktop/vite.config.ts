import { defineConfig } from 'vite';
import path from 'node:path';
import { readFileSync, existsSync, readdirSync } from 'node:fs';

const ROOT = path.resolve(import.meta.dirname, '../..');
const FIXTURES = path.join(ROOT, 'packages/render/fixtures');

// 浏览器里直接跑（没有 Tauri）时，用样例档案调界面；也可以用 CREDITIA_DEV_DIR 指向一组 */credits.json
const devSample = () => ({
  name: 'creditia-dev-sample',
  apply: 'serve' as const,
  configureServer(server: any) {
    const json = (res: any, body: unknown) => { res.setHeader('content-type', 'application/json'); res.end(JSON.stringify(body)); };
    const list = () => {
      const out: { id: string; file: string; name: string }[] = [];
      for (const f of existsSync(FIXTURES) ? readdirSync(FIXTURES) : []) if (f.endsWith('.json')) out.push({ id: f.replace('.json', ''), file: path.join(FIXTURES, f), name: '' });
      const extra = process.env.CREDITIA_DEV_DIR;
      if (extra && existsSync(extra)) for (const d of readdirSync(extra)) { const f = path.join(extra, d, 'credits.json'); if (existsSync(f)) out.push({ id: d, file: f, name: '' }); }
      return out.map((x) => ({ ...x, name: JSON.parse(readFileSync(x.file, 'utf8')).project?.name ?? x.id }));
    };
    server.middlewares.use('/__dev/projects', (_req: any, res: any) => json(res, list().map((x) => ({ id: x.id, name: x.name, path: `~/code/${x.name}`, paths: [`~/code/${x.name}`, `~/code/${x.name}-worktrees/feature`], lastAnalyzed: Math.floor(Date.now() / 1000), evidence: 18234, exists: true, hasCredits: true }))));
    server.middlewares.use('/__dev/credits', (req: any, res: any) => {
      const id = new URL(req.url, 'http://x').searchParams.get('id') ?? '';
      const hit = list().find((x) => x.id === id);
      if (!hit) { res.statusCode = 404; res.end('{}'); return; }
      // 调试用：CREDITIA_DEV_SLOW=<项目 id> 让这个项目的档案晚 3 秒返回，用来验证切项目不会串
      const slow = process.env.CREDITIA_DEV_SLOW === id ? 3000 : 0;
      setTimeout(() => { res.setHeader('content-type', 'application/json'); res.end(readFileSync(hit.file, 'utf8')); }, slow);
    });
    server.middlewares.use('/__dev/doctor', (_req: any, res: any) => json(res, [
      { id: 'claude-code', name: 'Claude Code', ok: true, detail: '样例数据' },
      { id: 'codex', name: 'Codex', ok: true, detail: '样例数据' },
      { id: 'cursor', name: 'Cursor', ok: false, detail: '没有找到' },
    ]));
  },
});

export default defineConfig({
  clearScreen: false,
  server: { port: 5183, strictPort: true, fs: { allow: [ROOT] } },
  build: { target: 'safari15', assetsInlineLimit: 0 },
  plugins: [devSample()],
});
