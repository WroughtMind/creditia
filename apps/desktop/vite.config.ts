import { defineConfig } from 'vite';
import path from 'node:path';
import os from 'node:os';
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
    // 和 Tauri 端 doctor() 同一套探测：看本机真实目录，不写假数据
    server.middlewares.use('/__dev/doctor', (_req: any, res: any) => {
      const home = os.homedir();
      const probe = (id: string, name: string, dir: string, marker: string) => {
        const ok = existsSync(path.join(dir, marker));
        return { id, name, path: dir, ok, detail: ok ? '已就绪' : '没有找到' };
      };
      json(res, JSON.stringify([
        probe('codex', 'Codex', process.env.CODEX_HOME ?? path.join(home, '.codex'), 'state_5.sqlite'),
        probe('claude-code', 'Claude Code', path.join(home, '.claude'), 'projects'),
        probe('cursor', 'Cursor', path.join(home, '.cursor'), 'ai-tracking/ai-code-tracking.db'),
        probe('grok', 'Grok', path.join(home, '.grok'), 'sessions'),
        probe('kimi', 'Kimi', existsSync(path.join(home, '.kimi-code')) ? path.join(home, '.kimi-code') : path.join(home, '.kimi'), 'sessions'),
      ]));
    });
  },
});

export default defineConfig({
  publicDir: path.resolve(ROOT, 'packages/render/assets/logos'),
  clearScreen: false,
  server: { port: 5183, strictPort: true, fs: { allow: [ROOT] } },
  build: { target: 'safari15' },
  plugins: [devSample()],
});
