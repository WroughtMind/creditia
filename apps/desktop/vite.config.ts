import { defineConfig } from 'vite';
import path from 'node:path';
import { readFileSync, existsSync, readdirSync } from 'node:fs';

const ROOT = path.resolve(import.meta.dirname, '../..');
const SAMPLES = path.join(ROOT, 'spike/out/core');

// 浏览器里直接跑（没有 Tauri）时，用正式核心产出的样例数据，方便调界面
const devSample = () => ({
  name: 'creditia-dev-sample',
  apply: 'serve' as const,
  configureServer(server: any) {
    const json = (res: any, body: string) => { res.setHeader('content-type', 'application/json'); res.end(body); };
    server.middlewares.use('/__dev/projects', (_req: any, res: any) => {
      const list: any[] = [];
      if (existsSync(SAMPLES)) for (const id of readdirSync(SAMPLES)) {
        const f = path.join(SAMPLES, id, 'credits.json');
        if (!existsSync(f)) continue;
        const c = JSON.parse(readFileSync(f, 'utf8'));
        list.push({ id, name: c.project?.name ?? id, path: path.join(SAMPLES, id), lastAnalyzed: Math.floor(Date.now() / 1000), evidence: 0, exists: true, hasCredits: true });
      }
      json(res, JSON.stringify(list));
    });
    server.middlewares.use('/__dev/credits', (req: any, res: any) => {
      const id = new URL(req.url, 'http://x').searchParams.get('id') ?? '';
      const f = path.join(SAMPLES, id, 'credits.json');
      if (!/^[\w.-]+$/.test(id) || !existsSync(f)) { res.statusCode = 404; res.end('{}'); return; }
      json(res, readFileSync(f, 'utf8'));
    });
    server.middlewares.use('/__dev/config', (_req: any, res: any) => json(res, '{}'));
    server.middlewares.use('/__dev/doctor', (_req: any, res: any) => json(res, JSON.stringify([
      { id: 'codex', name: 'Codex', path: '~/.codex', ok: true, detail: '样例数据（vite dev）' },
      { id: 'claude-code', name: 'Claude Code', path: '~/.claude', ok: true, detail: '样例数据（vite dev）' },
    ])));
  },
});

export default defineConfig({
  publicDir: path.resolve(ROOT, 'packages/render/assets/logos'),
  clearScreen: false,
  server: { port: 5183, strictPort: true, fs: { allow: [ROOT] } },
  build: { target: 'safari15' },
  plugins: [devSample()],
});
